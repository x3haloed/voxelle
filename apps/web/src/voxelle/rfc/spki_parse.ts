import { base64ToBytes } from './util_b64'

const ED25519_SPKI_PREFIX = new Uint8Array([
  0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
])

export function ed25519PublicKeyFromSpkiDer(spkiDer: Uint8Array): Uint8Array {
  if (spkiDer.byteLength !== 44) throw new Error(`SPKI DER must be 44 bytes for Ed25519 (got ${spkiDer.byteLength})`)
  for (let i = 0; i < ED25519_SPKI_PREFIX.length; i++) {
    if (spkiDer[i] !== ED25519_SPKI_PREFIX[i]) throw new Error('SPKI DER prefix mismatch (not Ed25519 SPKI)')
  }
  return spkiDer.slice(12, 44)
}

export function ed25519PublicKeyFromSpkiBase64(spkiB64: string): Uint8Array {
  return ed25519PublicKeyFromSpkiDer(base64ToBytes(spkiB64))
}

