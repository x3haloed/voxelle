import * as ed from '@noble/ed25519'
import type { DelegationCertV1 } from './types'
import { NetstringWriter } from './netstring'
import { base64ToBytes, bytesToBase64 } from './util_b64'
import { idFromSpkiDer } from './ids'

export async function createDelegationCert(params: {
  principal_sk_b64: string
  principal_id: string
  principal_pub_spki_b64: string
  device_pub_spki_b64: string
  device_id: string
  not_before_ts: number
  expires_ts: number
  scopes: string[]
}): Promise<DelegationCertV1> {
  const unsigned: Omit<DelegationCertV1, 'sig'> = {
    v: 1,
    principal_id: params.principal_id,
    principal_pub: params.principal_pub_spki_b64,
    device_pub: params.device_pub_spki_b64,
    device_id: params.device_id,
    not_before_ts: params.not_before_ts,
    expires_ts: params.expires_ts,
    scopes: params.scopes,
  }
  const sigInput = delegationSignatureInput(unsigned)
  const sk = base64ToBytes(params.principal_sk_b64)
  const sig = await ed.signAsync(sigInput, sk)
  return { ...unsigned, sig: bytesToBase64(sig) }
}

export async function deviceIdFromDevicePubSpkiB64(device_pub_spki_b64: string): Promise<string> {
  const der = base64ToBytes(device_pub_spki_b64)
  return idFromSpkiDer(der)
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

