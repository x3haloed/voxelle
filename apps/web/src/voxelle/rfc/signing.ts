import * as ed from '@noble/ed25519'
import { base64UrlNoPad } from './base64'
import { idFromSpkiDer } from './ids'
import { jcsBytes } from './jcs'
import { NetstringWriter } from './netstring'
import { ed25519SpkiDerFromPublicKey, spkiDerBase64FromEd25519PublicKey } from './spki'
import { sha256 } from './hash'
import type { DelegationCertV1, EventV1 } from './types'
import { base64ToBytes, bytesToBase64 } from './util_b64'

export type VoxelleIdentityV1 = {
  v: 1
  principal_sk_b64: string
  principal_pk_b64: string
  principal_pub_spki_b64: string
  principal_id: string
  device_sk_b64: string
  device_pk_b64: string
  device_pub_spki_b64: string
  device_id: string
  delegations_by_space: Record<string, DelegationCertV1>
}

function nowMs(): number {
  return Date.now()
}

export async function createIdentity(): Promise<VoxelleIdentityV1> {
  const principalSk = ed.utils.randomSecretKey()
  const principalPk = await ed.getPublicKeyAsync(principalSk)
  const principalSpkiDer = ed25519SpkiDerFromPublicKey(principalPk)
  const principalId = await idFromSpkiDer(principalSpkiDer)

  const deviceSk = ed.utils.randomSecretKey()
  const devicePk = await ed.getPublicKeyAsync(deviceSk)
  const deviceSpkiDer = ed25519SpkiDerFromPublicKey(devicePk)
  const deviceId = await idFromSpkiDer(deviceSpkiDer)

  return {
    v: 1,
    principal_sk_b64: bytesToBase64(principalSk),
    principal_pk_b64: bytesToBase64(principalPk),
    principal_pub_spki_b64: bytesToBase64(principalSpkiDer),
    principal_id: principalId,
    device_sk_b64: bytesToBase64(deviceSk),
    device_pk_b64: bytesToBase64(devicePk),
    device_pub_spki_b64: bytesToBase64(deviceSpkiDer),
    device_id: deviceId,
    delegations_by_space: {},
  }
}

export function loadIdentity(): VoxelleIdentityV1 | null {
  const raw = localStorage.getItem('voxelle.identity.v1')
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as VoxelleIdentityV1
    if (parsed?.v !== 1) return null
    return parsed
  } catch {
    return null
  }
}

export function saveIdentity(identity: VoxelleIdentityV1) {
  localStorage.setItem('voxelle.identity.v1', JSON.stringify(identity))
}

export async function ensureIdentity(): Promise<VoxelleIdentityV1> {
  const existing = loadIdentity()
  if (existing) return existing
  const id = await createIdentity()
  saveIdentity(id)
  return id
}

export async function ensureDelegationForSpace(
  identity: VoxelleIdentityV1,
  spaceId: string,
): Promise<{ identity: VoxelleIdentityV1; delegation: DelegationCertV1 }> {
  const existing = identity.delegations_by_space[spaceId]
  const now = nowMs()
  if (existing && existing.expires_ts > now + 60_000) {
    return { identity, delegation: existing }
  }

  const principalSk = base64ToBytes(identity.principal_sk_b64)
  const principalPk = base64ToBytes(identity.principal_pk_b64)
  const devicePk = base64ToBytes(identity.device_pk_b64)

  const principalPubSpkiB64 = spkiDerBase64FromEd25519PublicKey(principalPk)
  const devicePubSpkiB64 = spkiDerBase64FromEd25519PublicKey(devicePk)

  const notBefore = now - 10 * 60_000
  const expires = now + 30 * 24 * 60 * 60_000
  const scopes = [`space:${spaceId}:join`, `space:${spaceId}:post`, `space:${spaceId}:governance`]

  const unsigned = {
    v: 1 as const,
    principal_id: identity.principal_id,
    principal_pub: principalPubSpkiB64,
    device_pub: devicePubSpkiB64,
    device_id: identity.device_id,
    not_before_ts: notBefore,
    expires_ts: expires,
    scopes,
  }

  const sigInput = delegationSignatureInput(unsigned)
  const sig = await ed.signAsync(sigInput, principalSk)
  const delegation: DelegationCertV1 = { ...unsigned, sig: bytesToBase64(sig) }

  const principalSpkiDer = ed25519SpkiDerFromPublicKey(principalPk)
  const deviceSpkiDer = ed25519SpkiDerFromPublicKey(devicePk)
  const recomputedPrincipalId = await idFromSpkiDer(principalSpkiDer)
  const recomputedDeviceId = await idFromSpkiDer(deviceSpkiDer)
  if (recomputedPrincipalId !== identity.principal_id) throw new Error('principal_id mismatch')
  if (recomputedDeviceId !== identity.device_id) throw new Error('device_id mismatch')

  const ok = await ed.verifyAsync(sig, sigInput, principalPk)
  if (!ok) throw new Error('delegation signature did not verify')

  const nextIdentity: VoxelleIdentityV1 = {
    ...identity,
    delegations_by_space: { ...identity.delegations_by_space, [spaceId]: delegation },
  }
  saveIdentity(nextIdentity)
  return { identity: nextIdentity, delegation }
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

export async function createMsgPostEvent(params: {
  identity: VoxelleIdentityV1
  delegation: DelegationCertV1
  spaceId: string
  roomId: string
  prev: string[]
  text: string
}): Promise<EventV1> {
  const deviceSk = base64ToBytes(params.identity.device_sk_b64)
  const devicePk = base64ToBytes(params.identity.device_pk_b64)

  const ts = nowMs()
  const body = { text: params.text }
  const unsigned = {
    v: 1 as const,
    space_id: params.spaceId,
    room_id: params.roomId,
    author_principal_id: params.identity.principal_id,
    author_device_id: params.identity.device_id,
    author_device_pub: spkiDerBase64FromEd25519PublicKey(devicePk),
    delegation: params.delegation,
    ts,
    kind: 'MSG_POST',
    prev: [...params.prev].sort(),
    body,
  }

  const sigInput = eventSignatureInput(unsigned)
  const sig = await ed.signAsync(sigInput, deviceSk)
  const eventId = await eventIdFromSignatureInput(sigInput)

  const event: EventV1 = {
    ...unsigned,
    event_id: eventId,
    sig: bytesToBase64(sig),
  }

  const ok = await ed.verifyAsync(sig, sigInput, devicePk)
  if (!ok) throw new Error('event signature did not verify')

  if (event.event_id !== (await eventIdFromSignatureInput(sigInput))) {
    throw new Error('event_id mismatch')
  }

  return event
}

type UnsignedEventForSig = Omit<EventV1, 'sig' | 'event_id'>

function eventSignatureInput(unsigned: UnsignedEventForSig): Uint8Array {
  const w = new NetstringWriter()
  w.writePrefix('p2pspace/event/v0\n')
  w.writeInt(unsigned.v)
  w.writeStr(unsigned.space_id)
  w.writeStr(unsigned.room_id)
  w.writeStr(unsigned.author_principal_id)
  w.writeStr(unsigned.author_device_id)
  w.writeStr(unsigned.author_device_pub)
  w.writeStr(unsigned.delegation.sig)
  w.writeInt(unsigned.ts)
  w.writeStr(unsigned.kind)
  w.writeCount(unsigned.prev.length)
  for (const p of unsigned.prev) w.writeStr(p)
  w.writeBytes(jcsBytes(unsigned.body ?? {}))
  return w.finish()
}

async function eventIdFromSignatureInput(sigInput: Uint8Array): Promise<string> {
  const digest = await sha256(sigInput)
  return `e:${base64UrlNoPad(digest)}`
}
