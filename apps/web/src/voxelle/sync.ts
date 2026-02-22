import type { EventV1 } from './rfc/types'
import { acceptEvent } from './accept'
import { appendRoomEvent, getRoomEvents, getRoomHeads } from './store'
import type { JsonValue, WebRtcTransport } from './webrtc'

export type SyncMsg =
  | {
      t: 'hello'
      v: 1
      spaceId: string
      roomId: string
    }
  | {
      t: 'heads'
      v: 1
      spaceId: string
      roomId: string
      heads: string[]
    }
  | {
      t: 'want'
      v: 1
      spaceId: string
      roomId: string
      ids: string[]
    }
  | {
      t: 'have'
      v: 1
      spaceId: string
      roomId: string
      events: EventV1[]
    }

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null
}

function asSyncMsg(x: JsonValue): SyncMsg | null {
  if (!isObj(x)) return null
  if (x.v !== 1) return null
  const t = x.t
  if (t === 'hello' || t === 'heads' || t === 'want' || t === 'have') return x as unknown as SyncMsg
  return null
}

export function startRoomSync(params: {
  transport: WebRtcTransport
  spaceId: string
  roomId: string
  onLog?: (line: string) => void
}) {
  const log = (s: string) => params.onLog?.(s)
  const { transport, spaceId, roomId } = params

  const send = (msg: SyncMsg) => transport.send(msg as unknown as JsonValue)

  const sendHello = () => send({ t: 'hello', v: 1, spaceId, roomId })
  const sendHeads = () => send({ t: 'heads', v: 1, spaceId, roomId, heads: getRoomHeads(spaceId, roomId) })

  const knownIds = () => new Set(getRoomEvents(spaceId, roomId).map((e) => e.event_id))

  const computeMissingFromHeads = (heads: string[]) => {
    const have = knownIds()
    return heads.filter((id) => !have.has(id)).slice(0, 256)
  }

  const unsubMsg = transport.onMessage(async (m) => {
    const msg = asSyncMsg(m)
    if (!msg) return
    if (msg.spaceId !== spaceId || msg.roomId !== roomId) return

    if (msg.t === 'hello') {
      log('peer: hello')
      sendHeads()
      return
    }

    if (msg.t === 'heads') {
      log(`peer: heads(${msg.heads.length})`)
      const missing = computeMissingFromHeads(msg.heads)
      if (missing.length > 0) {
        log(`local: want(${missing.length})`)
        send({ t: 'want', v: 1, spaceId, roomId, ids: missing })
      }
      return
    }

    if (msg.t === 'want') {
      const have = new Map(getRoomEvents(spaceId, roomId).map((e) => [e.event_id, e]))
      const events: EventV1[] = []
      for (const id of msg.ids.slice(0, 256)) {
        const ev = have.get(id)
        if (ev) events.push(ev)
      }
      log(`peer: want(${msg.ids.length}) -> have(${events.length})`)
      if (events.length > 0) send({ t: 'have', v: 1, spaceId, roomId, events })
      return
    }

    if (msg.t === 'have') {
      let accepted = 0
      for (const ev of msg.events.slice(0, 256)) {
        const ok = await acceptEvent(ev, getRoomEvents)
        if (!ok.ok) continue
        appendRoomEvent(spaceId, roomId, ok.value)
        accepted++
      }
      log(`peer: have(${msg.events.length}) accepted(${accepted})`)
      return
    }
  })

  const onAppended = (ev: Event) => {
    const ce = ev as CustomEvent<any>
    const d = ce?.detail
    if (d?.v !== 1) return
    if (d.spaceId !== spaceId || d.roomId !== roomId) return

    const ids = [String(d.eventId)].slice(0, 1)
    const all = new Map(getRoomEvents(spaceId, roomId).map((e) => [e.event_id, e]))
    const events = ids.map((id) => all.get(id)).filter(Boolean) as EventV1[]
    if (events.length > 0) send({ t: 'have', v: 1, spaceId, roomId, events })
  }
  window.addEventListener('voxelle-room-event-appended', onAppended)

  sendHello()
  sendHeads()

  return () => {
    unsubMsg()
    window.removeEventListener('voxelle-room-event-appended', onAppended)
  }
}
