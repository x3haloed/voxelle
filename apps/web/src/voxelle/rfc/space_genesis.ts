import * as ed from '@noble/ed25519'
import { NetstringWriter } from './netstring'
import { idFromSpkiDer } from './ids'
import { base64ToBytes, bytesToBase64 } from './util_b64'
import { ed25519SpkiDerFromPublicKey, spkiDerBase64FromEd25519PublicKey } from './spki'
import { ed25519PublicKeyFromSpkiDer } from './spki_parse'

export type SpaceGenesisV1 = {
  v: 1
  space_id: string
  space_root_pub: string // SPKI DER base64 (RFC)
  created_ts: number
  name?: string
  sig: string // base64 ed25519 signature
}

export async function createSpaceRootKeypair(): Promise<{ sk_b64: string; pk_spki_b64: string; space_id: string }> {
  const sk = ed.utils.randomSecretKey()
  const pk32 = await ed.getPublicKeyAsync(sk)
  const spkiDer = ed25519SpkiDerFromPublicKey(pk32)
  const spaceId = await idFromSpkiDer(spkiDer)
  return {
    sk_b64: bytesToBase64(sk),
    pk_spki_b64: bytesToBase64(spkiDer),
    space_id: spaceId,
  }
}

export async function createSpaceGenesis(params: {
  space_root_sk_b64: string
  name?: string
  created_ts?: number
}): Promise<SpaceGenesisV1> {
  const created_ts = params.created_ts ?? Date.now()
  const sk = base64ToBytes(params.space_root_sk_b64)
  const pk32 = await ed.getPublicKeyAsync(sk)
  const space_root_pub = spkiDerBase64FromEd25519PublicKey(pk32)
  const spkiDer = base64ToBytes(space_root_pub)
  const space_id = await idFromSpkiDer(spkiDer)

  const unsigned = {
    v: 1 as const,
    space_id,
    space_root_pub,
    created_ts,
    name: params.name ?? '',
  }
  const sigInput = spaceGenesisSignatureInput(unsigned)
  const sig = await ed.signAsync(sigInput, sk)
  const genesis: SpaceGenesisV1 = {
    v: 1,
    space_id,
    space_root_pub,
    created_ts,
    name: params.name,
    sig: bytesToBase64(sig),
  }
  return genesis
}

export async function validateSpaceGenesis(genesis: SpaceGenesisV1): Promise<{ ok: true } | { ok: false; error: string }> {
  try {
    if (genesis.v !== 1) throw new Error('genesis.v must be 1')
    if (!genesis.space_id) throw new Error('missing space_id')
    if (!genesis.space_root_pub) throw new Error('missing space_root_pub')
    if (!genesis.sig) throw new Error('missing sig')

    const spkiDer = base64ToBytes(genesis.space_root_pub)
    const spaceId2 = await idFromSpkiDer(spkiDer)
    if (spaceId2 !== genesis.space_id) throw new Error('space_id does not match space_root_pub')

    const pk32 = ed25519PublicKeyFromSpkiDer(spkiDer)
    const sigInput = spaceGenesisSignatureInput({
      v: 1,
      space_id: genesis.space_id,
      space_root_pub: genesis.space_root_pub,
      created_ts: genesis.created_ts,
      name: genesis.name ?? '',
    })
    const sigBytes = base64ToBytes(genesis.sig)
    const ok = await ed.verifyAsync(sigBytes, sigInput, pk32)
    if (!ok) throw new Error('invalid signature')
    return { ok: true }
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) }
  }
}

function spaceGenesisSignatureInput(unsigned: {
  v: 1
  space_id: string
  space_root_pub: string
  created_ts: number
  name: string
}): Uint8Array {
  const w = new NetstringWriter()
  w.writePrefix('p2pspace/space-genesis/v0\n')
  w.writeInt(unsigned.v)
  w.writeStr(unsigned.space_id)
  w.writeStr(unsigned.space_root_pub)
  w.writeInt(unsigned.created_ts)
  w.writeStr(unsigned.name ?? '')
  return w.finish()
}

