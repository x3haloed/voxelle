import type { Room, Space } from './types'
import type { EventV1 } from './rfc/types'
import { computeHeads, topoSortDeterministic } from './dag'
import type { SpaceGenesisV1 } from './rfc/space_genesis'
import { createSpaceGenesis, createSpaceRootKeypair, validateSpaceGenesis } from './rfc/space_genesis'

type State = {
  spaces: Space[]
  rooms: Room[]
}

type SpaceRecordV1 = {
  v: 1
  space_id: string
  name: string
  genesis: SpaceGenesisV1
  space_root_sk_b64: string
}

const SPACES_KEY = 'voxelle.spaces.v1'
const ROOMS_KEY = 'voxelle.rooms.v1'

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
  const spaces = loadSpaceRecords()
  if (spaces.length > 0) return

  // Seed a couple spaces for development convenience.
  ;(async () => {
    await createSpace('Local Space')
    await createSpace('Voxelle Dev')
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

  const rec: SpaceRecordV1 = {
    v: 1,
    space_id: genesis.space_id,
    name: nm,
    genesis,
    space_root_sk_b64: kp.sk_b64,
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
