import * as ed from '@noble/ed25519'
import { idFromSpkiDer } from './ids'
import { base64ToBytes } from './util_b64'
import { ed25519PublicKeyFromSpkiBase64, ed25519PublicKeyFromSpkiDer } from './spki_parse'
import { NetstringWriter } from './netstring'
import { jcsBytes } from './jcs'
import { sha256 } from './hash'
import { base64UrlNoPad } from './base64'
import type { DelegationCertV1, EventV1 } from './types'

export type ValidationOk<T> = { ok: true; value: T }
export type ValidationErr = { ok: false; error: string }
export type ValidationResult<T> = ValidationOk<T> | ValidationErr

function nowMs(): number {
  return Date.now()
}

function requireString(x: unknown, name: string): string {
  if (typeof x !== 'string' || x.trim() === '') throw new Error(`${name} must be non-empty string`)
  return x
}

function requireNumber(x: unknown, name: string): number {
  if (typeof x !== 'number' || !Number.isFinite(x)) throw new Error(`${name} must be finite number`)
  return x
}

function requireArray(x: unknown, name: string): unknown[] {
  if (!Array.isArray(x)) throw new Error(`${name} must be array`)
  return x
}

function scopeForKind(kind: string, spaceId: string): string {
  if (kind === 'MEMBER_JOIN') return `space:${spaceId}:join`
  if (kind.startsWith('MSG_') || kind.startsWith('REACTION_') || kind.startsWith('PIN_')) {
    return `space:${spaceId}:post`
  }
  if (
    kind === 'SPACE_POLICY_SET' ||
    kind.startsWith('ROLE_') ||
    kind.startsWith('MEMBER_') ||
    kind.startsWith('INVITE_') ||
    kind.startsWith('ROOM_') ||
    kind.startsWith('DEVICE_')
  ) {
    return `space:${spaceId}:governance`
  }
  // Forward-compat: default to post requirement for unknown kinds.
  return `space:${spaceId}:post`
}

export async function validateDelegation(
  delegation: DelegationCertV1,
  expectedPrincipalId: string,
  expectedDeviceId: string,
  requiredScope: string,
): Promise<ValidationResult<DelegationCertV1>> {
  try {
    if (delegation.v !== 1) throw new Error('delegation.v must be 1')
    requireString(delegation.principal_id, 'delegation.principal_id')
    requireString(delegation.principal_pub, 'delegation.principal_pub')
    requireString(delegation.device_id, 'delegation.device_id')
    requireString(delegation.device_pub, 'delegation.device_pub')
    requireNumber(delegation.not_before_ts, 'delegation.not_before_ts')
    requireNumber(delegation.expires_ts, 'delegation.expires_ts')
    requireString(delegation.sig, 'delegation.sig')
    const scopes = requireArray(delegation.scopes, 'delegation.scopes').map((s) => {
      if (typeof s !== 'string') throw new Error('delegation.scopes entries must be strings')
      return s
    })

    if (delegation.principal_id !== expectedPrincipalId) throw new Error('delegation principal_id mismatch')
    if (delegation.device_id !== expectedDeviceId) throw new Error('delegation device_id mismatch')

    const principalSpkiDer = base64ToBytes(delegation.principal_pub)
    const deviceSpkiDer = base64ToBytes(delegation.device_pub)

    // Validate ids against SPKI bytes (RFC §4.3 + §5.2)
    const principalId2 = await idFromSpkiDer(principalSpkiDer)
    const deviceId2 = await idFromSpkiDer(deviceSpkiDer)
    if (principalId2 !== delegation.principal_id) throw new Error('delegation principal_id does not match principal_pub')
    if (deviceId2 !== delegation.device_id) throw new Error('delegation device_id does not match device_pub')

    // Validity window (allow ±10 minutes skew)
    const skew = 10 * 60_000
    const now = nowMs()
    if (now + skew < delegation.not_before_ts) throw new Error('delegation not yet valid')
    if (now - skew > delegation.expires_ts) throw new Error('delegation expired')

    if (!scopes.includes(requiredScope)) throw new Error(`delegation missing required scope: ${requiredScope}`)

    // Verify signature (RFC §7.3.2 delegation signature input)
    const sigInput = delegationSignatureInput({
      v: delegation.v,
      principal_id: delegation.principal_id,
      principal_pub: delegation.principal_pub,
      device_id: delegation.device_id,
      device_pub: delegation.device_pub,
      not_before_ts: delegation.not_before_ts,
      expires_ts: delegation.expires_ts,
      scopes,
    })
    const sigBytes = base64ToBytes(delegation.sig)
    const principalPk = ed25519PublicKeyFromSpkiDer(principalSpkiDer)
    const ok = await ed.verifyAsync(sigBytes, sigInput, principalPk)
    if (!ok) throw new Error('delegation signature invalid')

    return { ok: true, value: delegation }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

export async function validateEvent(ev: EventV1): Promise<ValidationResult<EventV1>> {
  try {
    if (ev.v !== 1) throw new Error('event.v must be 1')
    const spaceId = requireString(ev.space_id, 'space_id')
    requireString(ev.room_id, 'room_id')
    requireString(ev.kind, 'kind')
    requireString(ev.author_principal_id, 'author_principal_id')
    requireString(ev.author_device_id, 'author_device_id')
    requireString(ev.author_device_pub, 'author_device_pub')
    requireString(ev.sig, 'sig')
    requireString(ev.event_id, 'event_id')
    requireNumber(ev.ts, 'ts')
    const prev = requireArray(ev.prev, 'prev').map((p) => {
      if (typeof p !== 'string') throw new Error('prev entries must be strings')
      return p
    })

    const requiredScope = scopeForKind(ev.kind, spaceId)
    const delegationOk = await validateDelegation(ev.delegation, ev.author_principal_id, ev.author_device_id, requiredScope)
    if (!delegationOk.ok) throw new Error(`delegation invalid: ${delegationOk.error}`)

    // Verify event signature
    const sigInput = eventSignatureInput({
      v: ev.v,
      space_id: ev.space_id,
      room_id: ev.room_id,
      author_principal_id: ev.author_principal_id,
      author_device_id: ev.author_device_id,
      author_device_pub: ev.author_device_pub,
      delegation_sig: ev.delegation.sig,
      ts: ev.ts,
      kind: ev.kind,
      prev,
      body: ev.body,
    })

    const sigBytes = base64ToBytes(ev.sig)
    const devicePk = ed25519PublicKeyFromSpkiBase64(ev.author_device_pub)
    const ok = await ed.verifyAsync(sigBytes, sigInput, devicePk)
    if (!ok) throw new Error('event signature invalid')

    // Verify event_id = e:base64url(sha256(sigInput))
    const digest = await sha256(sigInput)
    const expectedEventId = `e:${base64UrlNoPad(digest)}`
    if (ev.event_id !== expectedEventId) throw new Error('event_id mismatch')

    return { ok: true, value: ev }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

function delegationSignatureInput(unsigned: Omit<DelegationCertV1, 'sig'>): Uint8Array {
  const w = new NetstringWriter()
  w.writePrefix('p2pspace/delegation/v0\n')
  w.writeInt(unsigned.v)
  w.writeStr(unsigned.principal_id)
  w.writeStr(unsigned.principal_pub)
  w.writeStr(unsigned.device_id)
  w.writeStr(unsigned.device_pub)
  w.writeInt(unsigned.not_before_ts)
  w.writeInt(unsigned.expires_ts)
  w.writeCount(unsigned.scopes.length)
  for (const scope of unsigned.scopes) w.writeStr(scope)
  return w.finish()
}

function eventSignatureInput(unsigned: {
  v: 1
  space_id: string
  room_id: string
  author_principal_id: string
  author_device_id: string
  author_device_pub: string
  delegation_sig: string
  ts: number
  kind: string
  prev: string[]
  body: unknown
}): Uint8Array {
  const w = new NetstringWriter()
  w.writePrefix('p2pspace/event/v0\n')
  w.writeInt(unsigned.v)
  w.writeStr(unsigned.space_id)
  w.writeStr(unsigned.room_id)
  w.writeStr(unsigned.author_principal_id)
  w.writeStr(unsigned.author_device_id)
  w.writeStr(unsigned.author_device_pub)
  w.writeStr(unsigned.delegation_sig)
  w.writeInt(unsigned.ts)
  w.writeStr(unsigned.kind)
  w.writeCount(unsigned.prev.length)
  for (const p of unsigned.prev) w.writeStr(p)
  w.writeBytes(jcsBytes(unsigned.body ?? {}))
  return w.finish()
}
