import * as ed from '@noble/ed25519'
import type { DelegationCertV1 } from './types'
import { NetstringWriter } from './netstring'
import { jcsBytes } from './jcs'
import { base64UrlNoPad } from './base64'
import { base64ToBytes, bytesToBase64 } from './util_b64'
import { ed25519PublicKeyFromSpkiBase64 } from './spki_parse'
import { idFromSpkiDer } from './ids'

export type InviteV1 = {
  v: 1
  space_id: string
  invite_id: string
  issued_ts: number
  expires_ts: number
  issuer_principal_id: string
  issuer_device_id: string
  issuer_device_pub: string // SPKI DER Base64
  issuer_delegation: DelegationCertV1
  invite_issuer?: unknown
  scopes: string[]
  constraints?: unknown
  bootstrap: unknown
  sig: string
}

export function newInviteId(): string {
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  return base64UrlNoPad(bytes)
}

export async function createInvite(params: {
  space_id: string
  issuer_principal_id: string
  issuer_device_id: string
  issuer_device_pub: string
  issuer_delegation: DelegationCertV1
  issuer_device_sk_b64: string
  scopes: string[]
  expires_ts: number
  constraints?: unknown
  bootstrap: unknown
  issued_ts?: number
  invite_id?: string
}): Promise<InviteV1> {
  const issued_ts = params.issued_ts ?? Date.now()
  const invite_id = params.invite_id ?? newInviteId()

  const unsigned = {
    v: 1 as const,
    space_id: params.space_id,
    invite_id,
    issued_ts,
    expires_ts: params.expires_ts,
    issuer_principal_id: params.issuer_principal_id,
    issuer_device_id: params.issuer_device_id,
    issuer_device_pub: params.issuer_device_pub,
    issuer_delegation: params.issuer_delegation,
    invite_issuer: undefined as unknown,
    scopes: params.scopes,
    constraints: params.constraints ?? {},
    bootstrap: params.bootstrap ?? {},
  }

  const sigInput = inviteSignatureInput(unsigned)
  const sk = base64ToBytes(params.issuer_device_sk_b64)
  const sig = await ed.signAsync(sigInput, sk)

  return {
    ...unsigned,
    sig: bytesToBase64(sig),
  }
}

export async function validateInvite(invite: InviteV1): Promise<{ ok: true } | { ok: false; error: string }> {
  try {
    if (invite.v !== 1) throw new Error('invite.v must be 1')
    if (!invite.space_id) throw new Error('missing space_id')
    if (!invite.invite_id) throw new Error('missing invite_id')
    if (!invite.sig) throw new Error('missing sig')
    if (!Array.isArray(invite.scopes) || invite.scopes.length === 0) throw new Error('missing scopes')
    if (!invite.scopes.includes(`space:${invite.space_id}:read`)) throw new Error('invite missing required read scope')

    const now = Date.now()
    if (now > invite.expires_ts) throw new Error('invite expired')

    // Validate delegation + ids
    if (!invite.issuer_delegation) throw new Error('missing issuer_delegation')
    if (invite.issuer_delegation.device_id !== invite.issuer_device_id) throw new Error('issuer_delegation device_id mismatch')
    if (invite.issuer_delegation.principal_id !== invite.issuer_principal_id) throw new Error(
      'issuer_delegation principal_id mismatch',
    )

    // If invite_issuer is missing, issuer must be space root (RFC §8.5)
    if (invite.invite_issuer == null && invite.issuer_principal_id !== invite.space_id) {
      throw new Error('invite_issuer missing and issuer_principal_id != space_id (not space root)')
    }

    // Verify ids match public keys
    const principalSpkiDer = base64ToBytes(invite.issuer_delegation.principal_pub)
    const principalId2 = await idFromSpkiDer(principalSpkiDer)
    if (principalId2 !== invite.issuer_principal_id) throw new Error('issuer_principal_id mismatch principal_pub')

    const deviceSpkiDer = base64ToBytes(invite.issuer_delegation.device_pub)
    const deviceId2 = await idFromSpkiDer(deviceSpkiDer)
    if (deviceId2 !== invite.issuer_device_id) throw new Error('issuer_device_id mismatch device_pub')

    // Verify invite signature
    const sigInput = inviteSignatureInput(invite)
    const sigBytes = base64ToBytes(invite.sig)
    const devicePk32 = ed25519PublicKeyFromSpkiBase64(invite.issuer_device_pub)
    const ok = await ed.verifyAsync(sigBytes, sigInput, devicePk32)
    if (!ok) throw new Error('invalid invite signature')

    return { ok: true }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

function inviteSignatureInput(unsigned: {
  v: 1
  space_id: string
  invite_id: string
  issued_ts: number
  expires_ts: number
  issuer_principal_id: string
  issuer_device_id: string
  issuer_device_pub: string
  issuer_delegation: DelegationCertV1
  invite_issuer?: any
  constraints?: any
  bootstrap: any
  sig?: string
  scopes?: string[]
}): Uint8Array {
  const w = new NetstringWriter()
  w.writePrefix('p2pspace/invite/v0\n')
  w.writeInt(unsigned.v)
  w.writeStr(unsigned.space_id)
  w.writeStr(unsigned.invite_id)
  w.writeInt(unsigned.issued_ts)
  w.writeInt(unsigned.expires_ts)
  w.writeStr(unsigned.issuer_principal_id)
  w.writeStr(unsigned.issuer_device_id)
  w.writeStr(unsigned.issuer_device_pub)
  w.writeStr(unsigned.issuer_delegation.sig)
  // invite_issuer.sig empty if missing (RFC)
  const iicSig = (unsigned.invite_issuer as any)?.sig
  w.writeStr(typeof iicSig === 'string' ? iicSig : '')
  w.writeBytes(jcsBytes(unsigned.constraints ?? {}))
  w.writeBytes(jcsBytes(unsigned.bootstrap ?? {}))
  return w.finish()
}
