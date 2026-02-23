import type { EventV1 } from './rfc/types'
import { acceptEvent } from './accept'
import { MAX_SYNC_HEADS, MAX_SYNC_HAVE_EVENTS, MAX_SYNC_WANT_IDS, TokenBucket } from './limits'
import { appendRoomEvent, getRoomEvents, getRoomHeads } from './store'
import type { JsonValue, WebRtcTransport } from './webrtc'

export type SyncStats = {
  phase: 'starting' | 'running' | 'stopped'
  lastPeerHelloTs?: number
  lastPeerHeadsCount?: number
  lastWantCount?: number
  lastHaveReceived?: number
  lastHaveAccepted?: number
  lastSentHeadsTs?: number
  lastSentHaveTs?: number
}

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

function asStr(x: unknown, maxLen: number): string | null {
  if (typeof x !== 'string') return null
  const s = x.trim()
  if (!s) return null
  if (s.length > maxLen) return null
  return s
}

function asStrArray(x: unknown, maxCount: number, maxItemLen: number): string[] | null {
  if (!Array.isArray(x)) return null
  if (x.length > maxCount) return null
  const out: string[] = []
  for (const it of x) {
    const s = asStr(it, maxItemLen)
    if (!s) return null
    out.push(s)
  }
  return out
}

function asEventArray(x: unknown, maxCount: number): EventV1[] | null {
  if (!Array.isArray(x)) return null
  if (x.length > maxCount) return null
  return x as unknown as EventV1[]
}

function asSyncMsg(x: JsonValue): SyncMsg | null {
  if (!isObj(x)) return null
  if (x.v !== 1) return null
  const t = x.t
  const spaceId = asStr(x.spaceId, 256)
  const roomId = asStr(x.roomId, 256)
  if (!spaceId || !roomId) return null

  if (t === 'hello') return { t: 'hello', v: 1, spaceId, roomId }
  if (t === 'heads') {
    const heads = asStrArray((x as any).heads, MAX_SYNC_HEADS, 256)
    if (!heads) return null
    return { t: 'heads', v: 1, spaceId, roomId, heads }
  }
  if (t === 'want') {
    const ids = asStrArray((x as any).ids, MAX_SYNC_WANT_IDS, 256)
    if (!ids) return null
    return { t: 'want', v: 1, spaceId, roomId, ids }
  }
  if (t === 'have') {
    const events = asEventArray((x as any).events, MAX_SYNC_HAVE_EVENTS)
    if (!events) return null
    return { t: 'have', v: 1, spaceId, roomId, events }
  }
  return null
}

export function startRoomSync(params: {
  transport: WebRtcTransport
  spaceId: string
  roomId: string
  onLog?: (line: string) => void
  onStats?: (stats: SyncStats) => void
}) {
  const log = (s: string) => params.onLog?.(s)
  const { transport, spaceId, roomId } = params

  let stats: SyncStats = { phase: 'starting' }
  const emitStats = (patch: Partial<SyncStats>) => {
    stats = { ...stats, ...patch }
    params.onStats?.(stats)
  }

  const send = (msg: SyncMsg) => transport.send(msg as unknown as JsonValue)

  const sendHello = () => send({ t: 'hello', v: 1, spaceId, roomId })
  const sendHeads = () => {
    emitStats({ lastSentHeadsTs: Date.now() })
    send({ t: 'heads', v: 1, spaceId, roomId, heads: getRoomHeads(spaceId, roomId) })
  }

  const known = new Set(getRoomEvents(spaceId, roomId).map((e) => e.event_id))
  const msgBudget = new TokenBucket(60, 20) // burst 60, ~20 msgs/sec
  const verifyBudget = new TokenBucket(80, 20) // burst 80, ~20 sig checks/sec
  let lastWarnTs = 0

  const warn = (s: string) => {
    const now = Date.now()
    if (now - lastWarnTs < 1000) return
    lastWarnTs = now
    log(s)
  }

  const computeMissingFromHeads = (heads: string[]) => {
    return heads.filter((id) => !known.has(id)).slice(0, MAX_SYNC_WANT_IDS)
  }

  const unsubMsg = transport.onMessage(async (m) => {
    const msg = asSyncMsg(m)
    if (!msg) return
    if (msg.spaceId !== spaceId || msg.roomId !== roomId) return
    if (!msgBudget.allow(1)) {
      warn('peer: rate limited')
      return
    }

    if (msg.t === 'hello') {
      log('peer: hello')
      emitStats({ phase: 'running', lastPeerHelloTs: Date.now() })
      sendHeads()
      return
    }

    if (msg.t === 'heads') {
      log(`peer: heads(${msg.heads.length})`)
      emitStats({ phase: 'running', lastPeerHeadsCount: msg.heads.length })
      const missing = computeMissingFromHeads(msg.heads)
      if (missing.length > 0) {
        log(`local: want(${missing.length})`)
        emitStats({ lastWantCount: missing.length })
        send({ t: 'want', v: 1, spaceId, roomId, ids: missing })
      }
      return
    }

    if (msg.t === 'want') {
      const have = new Map(getRoomEvents(spaceId, roomId).map((e) => [e.event_id, e]))
      const events: EventV1[] = []
      for (const id of msg.ids.slice(0, MAX_SYNC_WANT_IDS)) {
        const ev = have.get(id)
        if (ev) events.push(ev)
      }
      log(`peer: want(${msg.ids.length}) -> have(${events.length})`)
      if (events.length > 0) {
        emitStats({ lastSentHaveTs: Date.now() })
        send({ t: 'have', v: 1, spaceId, roomId, events })
      }
      return
    }

    if (msg.t === 'have') {
      let accepted = 0
      for (const ev of msg.events.slice(0, MAX_SYNC_HAVE_EVENTS)) {
        const eid = (ev as any)?.event_id
        if (typeof eid === 'string' && known.has(eid)) continue
        if (!verifyBudget.allow(1)) {
          warn('peer: verification rate limited')
          break
        }
        const ok = await acceptEvent(ev, getRoomEvents)
        if (!ok.ok) continue
        known.add(ok.value.event_id)
        appendRoomEvent(spaceId, roomId, ok.value)
        accepted++
      }
      log(`peer: have(${msg.events.length}) accepted(${accepted})`)
      emitStats({ lastHaveReceived: msg.events.length, lastHaveAccepted: accepted })
      return
    }
  })

  const onAppended = (ev: Event) => {
    const ce = ev as CustomEvent<any>
    const d = ce?.detail
    if (d?.v !== 1) return
    if (d.spaceId !== spaceId || d.roomId !== roomId) return

    known.add(String(d.eventId))

    const ids = [String(d.eventId)].slice(0, 1)
    const all = new Map(getRoomEvents(spaceId, roomId).map((e) => [e.event_id, e]))
    const events = ids.map((id) => all.get(id)).filter(Boolean) as EventV1[]
    if (events.length > 0) {
      emitStats({ lastSentHaveTs: Date.now() })
      send({ t: 'have', v: 1, spaceId, roomId, events })
    }
  }
  window.addEventListener('voxelle-room-event-appended', onAppended)

  emitStats({ phase: 'running' })
  sendHello()
  sendHeads()

  return () => {
    unsubMsg()
    window.removeEventListener('voxelle-room-event-appended', onAppended)
    emitStats({ phase: 'stopped' })
  }
}
