import type { EventV1 } from './rfc/types'
import type { Message } from './types'
import { topoSortDeterministic } from './dag'

export function messagesFromEvents(events: EventV1[]): Message[] {
  const byId = new Map(events.map((e) => [e.event_id, e]))
  const order = topoSortDeterministic(events)
  const out: Message[] = []

  for (const id of order) {
    const ev = byId.get(id)
    if (!ev) continue
    if (ev.kind !== 'MSG_POST') continue
    const body = ev.body as any
    const text = typeof body?.text === 'string' ? body.text : ''
    out.push({
      id: ev.event_id,
      spaceId: ev.space_id,
      roomId: ev.room_id,
      author: tiny(ev.author_principal_id),
      text,
      ts: ev.ts,
      meta: {
        eventId: ev.event_id,
        principalId: ev.author_principal_id,
        deviceId: ev.author_device_id,
      },
    })
  }
  return out
}

function tiny(id: string): string {
  if (!id) return 'unknown'
  if (id.length <= 24) return id
  return `${id.slice(0, 12)}…${id.slice(-8)}`
}
