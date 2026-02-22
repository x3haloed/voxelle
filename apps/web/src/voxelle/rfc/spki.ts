import { base64Encode } from './base64'

export function ed25519SpkiDerFromPublicKey(pk32: Uint8Array): Uint8Array {
  if (pk32.byteLength !== 32) throw new Error('ed25519 public key must be 32 bytes')
  // SubjectPublicKeyInfo for Ed25519:
  // SEQUENCE {
  //   SEQUENCE { OBJECT IDENTIFIER 1.3.101.112 }
  //   BIT STRING 0x00 || pk
  // }
  const out = new Uint8Array(44)
  out.set(
    [
      0x30,
      0x2a,
      0x30,
      0x05,
      0x06,
      0x03,
      0x2b,
      0x65,
      0x70,
      0x03,
      0x21,
      0x00,
    ],
    0,
  )
  out.set(pk32, 12)
  return out
}

export function spkiDerBase64FromEd25519PublicKey(pk32: Uint8Array): string {
  return base64Encode(ed25519SpkiDerFromPublicKey(pk32))
}

