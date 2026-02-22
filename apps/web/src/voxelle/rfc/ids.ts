import { base64UrlNoPad } from './base64'
import { sha256 } from './hash'

export async function idFromSpkiDer(spkiDer: Uint8Array): Promise<string> {
  const digest = await sha256(spkiDer)
  return `ed25519:${base64UrlNoPad(digest)}`
}

