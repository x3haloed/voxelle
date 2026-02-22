import type { Room, Space } from './types'
import type { EventV1 } from './rfc/types'

type State = {
  spaces: Space[]
  rooms: Room[]
}

let state: State = {
  spaces: [
    { id: 'space:local', name: 'Local Space' },
    { id: 'space:voxelle', name: 'Voxelle Dev' },
  ],
  rooms: [
    { id: 'room:general', spaceId: 'space:local', name: 'general', visibility: 'public' },
    { id: 'room:governance', spaceId: 'space:local', name: 'governance', visibility: 'public' },
    { id: 'room:general', spaceId: 'space:voxelle', name: 'general', visibility: 'public' },
    { id: 'room:design', spaceId: 'space:voxelle', name: 'design', visibility: 'public' },
  ],
}

export function getState(): State {
  return state
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
  const next = [...existing, ev]
  localStorage.setItem(keyForRoom(spaceId, roomId), JSON.stringify(next))
}
