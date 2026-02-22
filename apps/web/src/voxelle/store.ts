import type { Room, Space } from './types'
import type { EventV1 } from './rfc/types'
import { computeHeads, topoSortDeterministic } from './dag'
import type { SpaceGenesisV1 } from './rfc/space_genesis'
import { createSpaceGenesis, createSpaceRootKeypair, validateSpaceGenesis } from './rfc/space_genesis'
import type { InviteV1 } from './rfc/invite'
import { createInvite, newInviteId, validateInvite } from './rfc/invite'
import { ensureIdentity, ensureDelegationForSpace, createEventV1 } from './rfc/signing'
import { acceptEvent } from './accept'
import { createDelegationCert, deviceIdFromDevicePubSpkiB64 } from './rfc/delegation'
import { bytesToBase64 } from './rfc/util_b64'
import { spkiDerBase64FromEd25519PublicKey } from './rfc/spki'
import * as ed from '@noble/ed25519'
import { secretGet, secretSet, secretsAvailable } from './secrets'

type State = {
  spaces: Space[]
  rooms: Room[]
}

type SpaceRecordV1 = {
  v: 1
  space_id: string
  name: string
  genesis?: SpaceGenesisV1
  owner: boolean
  space_root_sk_b64?: string
  space_root_device_sk_b64?: string
  space_root_device_pk_b64?: string
  space_root_device_pub_spki_b64?: string
  space_root_device_id?: string
  space_root_device_delegation?: import('./rfc/types').DelegationCertV1
}

const SPACES_KEY = 'voxelle.spaces.v1'
const ROOMS_KEY = 'voxelle.rooms.v1'
const SPACE_ROOT_SK_KEY_PREFIX = 'space.'

function emitStateChanged() {
  window.dispatchEvent(new CustomEvent('voxelle-state-changed', { detail: { v: 1 } }))
}

export function getState(): State {
  ensureSeeded()
  return {
    spaces: loadSpaceRecords().map((r) => ({ id: r.space_id, name: r.name })),
    rooms: loadRooms(),
  }
}

export function onStateChanged(fn: () => void): () => void {
  const handler = () => fn()
  window.addEventListener('voxelle-state-changed', handler)
  return () => window.removeEventListener('voxelle-state-changed', handler)
}

function ensureSeeded() {
  if (localStorage.getItem('voxelle.seeded.v1') === '1') return
  const spaces = loadSpaceRecords()
  if (spaces.length > 0) return

  // Seed a couple spaces for development convenience.
  ;(async () => {
    await createSpace('Local Space')
    await createSpace('Voxelle Dev')
    localStorage.setItem('voxelle.seeded.v1', '1')
  })().catch(() => {
    // ignore seed errors
  })
}

function loadSpaceRecords(): SpaceRecordV1[] {
  const raw = localStorage.getItem(SPACES_KEY)
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed.filter((x) => x?.v === 1 && typeof x.space_id === 'string') as SpaceRecordV1[]
  } catch {
    return []
  }
}

function saveSpaceRecords(records: SpaceRecordV1[]) {
  localStorage.setItem(SPACES_KEY, JSON.stringify(records))
  emitStateChanged()
}

function replaceSpaceRecord(spaceId: string, next: SpaceRecordV1) {
  const all = loadSpaceRecords()
  const out: SpaceRecordV1[] = []
  let replaced = false
  for (const r of all) {
    if (r.space_id === spaceId) {
      out.push(next)
      replaced = true
    } else out.push(r)
  }
  if (!replaced) out.push(next)
  saveSpaceRecords(out)
}

function spaceSecretKey(spaceId: string, name: 'space_root_sk_b64' | 'space_root_device_sk_b64'): string {
  return `${SPACE_ROOT_SK_KEY_PREFIX}${encodeURIComponent(spaceId)}.${name}`
}

async function getOwnerSpaceSecrets(spaceId: string): Promise<{
  space_root_sk_b64?: string
  space_root_device_sk_b64?: string
}> {
  const rec = getSpaceRecord(spaceId)
  if (!rec || !rec.owner) throw new Error('not an owner space')

  if (!secretsAvailable()) {
    return { space_root_sk_b64: rec.space_root_sk_b64, space_root_device_sk_b64: rec.space_root_device_sk_b64 }
  }

  // Migrate any legacy secrets still present in localStorage into keychain.
  const toPersist: Array<[string, string]> = []
  if (rec.space_root_sk_b64?.trim()) toPersist.push([spaceSecretKey(spaceId, 'space_root_sk_b64'), rec.space_root_sk_b64])
  if (rec.space_root_device_sk_b64?.trim())
    toPersist.push([spaceSecretKey(spaceId, 'space_root_device_sk_b64'), rec.space_root_device_sk_b64])
  if (toPersist.length > 0) {
    for (const [k, v] of toPersist) {
      try {
        await secretSet(k, v)
      } catch {
        // If keychain write fails, keep legacy storage (better than losing the key).
        return { space_root_sk_b64: rec.space_root_sk_b64, space_root_device_sk_b64: rec.space_root_device_sk_b64 }
      }
    }
    const next: SpaceRecordV1 = {
      ...rec,
      space_root_sk_b64: undefined,
      space_root_device_sk_b64: undefined,
    }
    replaceSpaceRecord(spaceId, next)
  }

  const rootSk = await secretGet(spaceSecretKey(spaceId, 'space_root_sk_b64'))
  const deviceSk = await secretGet(spaceSecretKey(spaceId, 'space_root_device_sk_b64'))
  return {
    space_root_sk_b64: rootSk ?? undefined,
    space_root_device_sk_b64: deviceSk ?? undefined,
  }
}

function loadRooms(): Room[] {
  const raw = localStorage.getItem(ROOMS_KEY)
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed as Room[]
  } catch {
    return []
  }
}

function saveRooms(rooms: Room[]) {
  localStorage.setItem(ROOMS_KEY, JSON.stringify(rooms))
  emitStateChanged()
}

export function roomsForSpace(spaceId: string): Room[] {
  return loadRooms().filter((r) => r.spaceId === spaceId)
}

export async function createSpace(name: string): Promise<Space> {
  const nm = name.trim() || 'Untitled Space'
  const kp = await createSpaceRootKeypair()
  const genesis = await createSpaceGenesis({ space_root_sk_b64: kp.sk_b64, name: nm })
  const valid = await validateSpaceGenesis(genesis)
  if (!valid.ok) throw new Error(`genesis invalid: ${valid.error}`)

  // Create a "space root device" keypair + delegation for issuing invites (root principal should not be used directly).
  const rootDeviceSk = ed.utils.randomSecretKey()
  const rootDevicePk32 = await ed.getPublicKeyAsync(rootDeviceSk)
  const rootDevicePubSpkiB64 = spkiDerBase64FromEd25519PublicKey(rootDevicePk32)
  const rootDeviceId = await deviceIdFromDevicePubSpkiB64(rootDevicePubSpkiB64)
  const now = Date.now()
  const rootDelegation = await createDelegationCert({
    principal_sk_b64: kp.sk_b64,
    principal_id: genesis.space_id,
    principal_pub_spki_b64: genesis.space_root_pub,
    device_pub_spki_b64: rootDevicePubSpkiB64,
    device_id: rootDeviceId,
    not_before_ts: now - 10 * 60_000,
    expires_ts: now + 365 * 24 * 60 * 60_000,
    scopes: [`space:${genesis.space_id}:join`, `space:${genesis.space_id}:post`, `space:${genesis.space_id}:governance`],
  })

  const rec: SpaceRecordV1 = {
    v: 1,
    space_id: genesis.space_id,
    name: nm,
    genesis,
    owner: true,
    space_root_sk_b64: kp.sk_b64,
    space_root_device_sk_b64: bytesToBase64(rootDeviceSk),
    space_root_device_pk_b64: bytesToBase64(rootDevicePk32),
    space_root_device_pub_spki_b64: rootDevicePubSpkiB64,
    space_root_device_id: rootDeviceId,
    space_root_device_delegation: rootDelegation,
  }

  // Desktop: store secrets in OS keychain and avoid keeping them in localStorage.
  if (secretsAvailable()) {
    try {
      await secretSet(spaceSecretKey(rec.space_id, 'space_root_sk_b64'), kp.sk_b64)
      await secretSet(spaceSecretKey(rec.space_id, 'space_root_device_sk_b64'), bytesToBase64(rootDeviceSk))
      rec.space_root_sk_b64 = undefined
      rec.space_root_device_sk_b64 = undefined
    } catch {
      // If keychain write fails, keep legacy localStorage values (better than losing keys).
    }
  }

  const all = loadSpaceRecords()
  if (!all.some((r) => r.space_id === rec.space_id)) {
    saveSpaceRecords([...all, rec])
  }

  // Ensure default rooms exist (RFC governance room id is "governance").
  const rooms = loadRooms()
  const defaults: Room[] = [
    { id: 'governance', spaceId: rec.space_id, name: 'governance', visibility: 'public' },
    { id: 'room:general', spaceId: rec.space_id, name: 'general', visibility: 'public' },
  ]
  const nextRooms = [...rooms]
  for (const r of defaults) {
    if (!nextRooms.some((x) => x.spaceId === r.spaceId && x.id === r.id)) nextRooms.push(r)
  }
  saveRooms(nextRooms)

  return { id: rec.space_id, name: rec.name }
}

export function exportSpaceGenesis(spaceId: string): SpaceGenesisV1 | null {
  const rec = loadSpaceRecords().find((r) => r.space_id === spaceId)
  return rec?.genesis ?? null
}

export function getSpaceRecord(spaceId: string): SpaceRecordV1 | null {
  return loadSpaceRecords().find((r) => r.space_id === spaceId) ?? null
}

export function isSpaceOwner(spaceId: string): boolean {
  const r = getSpaceRecord(spaceId)
  return !!r?.owner
}

export async function joinSpaceFromInvite(invite: InviteV1): Promise<Space> {
  const ok = await validateInvite(invite)
  if (!ok.ok) throw new Error(`invalid invite: ${ok.error}`)

  // Create or update local space record (non-owner).
  const name = (invite.bootstrap as any)?.space_name
  const nm = typeof name === 'string' && name.trim() ? name.trim() : `Space ${invite.space_id.slice(0, 16)}…`

  const all = loadSpaceRecords()
  if (!all.some((r) => r.space_id === invite.space_id)) {
    saveSpaceRecords([
      ...all,
      {
        v: 1,
        space_id: invite.space_id,
        name: nm,
        owner: false,
      },
    ])
  }

  // Ensure the governance + general rooms exist locally.
  const rooms = loadRooms()
  const defaults: Room[] = [
    { id: 'governance', spaceId: invite.space_id, name: 'governance', visibility: 'public' },
    { id: 'room:general', spaceId: invite.space_id, name: 'general', visibility: 'public' },
  ]
  const nextRooms = [...rooms]
  for (const r of defaults) {
    if (!nextRooms.some((x) => x.spaceId === r.spaceId && x.id === r.id)) nextRooms.push(r)
  }
  saveRooms(nextRooms)

  // Emit MEMBER_JOIN in governance room (RFC).
  const identity0 = await ensureIdentity()
  const { identity, delegation } = await ensureDelegationForSpace(identity0, invite.space_id)

  const governanceRoomId = 'governance'
  const prev = computeHeads(getRoomEvents(invite.space_id, governanceRoomId)).slice(0, 8)
  const ev = await createEventV1({
    identity,
    delegation,
    spaceId: invite.space_id,
    roomId: governanceRoomId,
    prev,
    kind: 'MEMBER_JOIN',
    body: {
      principal_id: identity.principal_id,
      principal_pub: identity.principal_pub_spki_b64,
      invite,
    },
  })
  const accepted = await acceptEvent(ev, getRoomEvents)
  if (!accepted.ok) throw new Error(`join rejected: ${accepted.error}`)
  appendRoomEvent(invite.space_id, governanceRoomId, accepted.value)

  return { id: invite.space_id, name: nm }
}

export async function issueInviteFromOwner(params: {
  spaceId: string
  spaceNameHint?: string
  relayWsUrl?: string
  expiresInHours?: number
  allowPost?: boolean
}): Promise<InviteV1> {
  const rec = getSpaceRecord(params.spaceId)
  if (!rec || !rec.owner) throw new Error('not an owner space')

  const secrets = await getOwnerSpaceSecrets(params.spaceId)
  const spaceRootDeviceSkB64 = secrets.space_root_device_sk_b64

  if (!spaceRootDeviceSkB64 || !rec.space_root_device_delegation || !rec.space_root_device_id || !rec.space_root_device_pub_spki_b64) {
    throw new Error('space root device not configured')
  }

  const invite_id = newInviteId()
  const issued_ts = Date.now()
  const hours = params.expiresInHours ?? 24 * 7
  const expires_ts = issued_ts + Math.max(1, hours) * 60 * 60_000
  const scopes = [`space:${rec.space_id}:read`]
  if (params.allowPost ?? true) scopes.push(`space:${rec.space_id}:post`)

  const bootstrap = {
    space_name: params.spaceNameHint ?? rec.name,
    rendezvous: params.relayWsUrl?.trim()
      ? [`signal-ws:${params.relayWsUrl.trim()}#sid=${invite_id}`]
      : ['voxelle:oob'],
  }

  return createInvite({
    space_id: rec.space_id,
    issuer_principal_id: rec.space_id, // space root principal id == space_id
    issuer_device_id: rec.space_root_device_id,
    issuer_device_pub: rec.space_root_device_pub_spki_b64,
    issuer_delegation: rec.space_root_device_delegation,
    issuer_device_sk_b64: spaceRootDeviceSkB64,
    scopes,
    expires_ts,
    constraints: { principal_type: 'any' },
    bootstrap,
    issued_ts,
    invite_id,
  })
}

function keyForRoom(spaceId: string, roomId: string): string {
  return `voxelle.events.v1.${encodeURIComponent(spaceId)}.${encodeURIComponent(roomId)}`
}

export function getRoomEvents(spaceId: string, roomId: string): EventV1[] {
  const raw = localStorage.getItem(keyForRoom(spaceId, roomId))
  if (!raw) return []
  try {
    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    return parsed as EventV1[]
  } catch {
    return []
  }
}

export function appendRoomEvent(spaceId: string, roomId: string, ev: EventV1): void {
  const existing = getRoomEvents(spaceId, roomId)
  if (existing.some((e) => e?.event_id === ev.event_id)) return
  const next = [...existing, ev]
  localStorage.setItem(keyForRoom(spaceId, roomId), JSON.stringify(next))
  window.dispatchEvent(
    new CustomEvent('voxelle-room-event-appended', {
      detail: { v: 1, spaceId, roomId, eventId: ev.event_id },
    }),
  )
}

export function getRoomHeads(spaceId: string, roomId: string): string[] {
  return computeHeads(getRoomEvents(spaceId, roomId))
}

export function getRoomEventOrder(spaceId: string, roomId: string): string[] {
  return topoSortDeterministic(getRoomEvents(spaceId, roomId))
}
