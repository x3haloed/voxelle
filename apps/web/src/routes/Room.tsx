import { useEffect, useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { getState, getRoomEvents, appendRoomEvent } from '../voxelle/store'
import { messagesFromEvents } from '../voxelle/events'
import { ensureIdentity, ensureDelegationForSpace, createMsgPostEvent } from '../voxelle/rfc/signing'
import { validateEvent } from '../voxelle/rfc/validate'
import type { EventV1 } from '../voxelle/rfc/types'
import { computeHeads } from '../voxelle/dag'
import { ConnectionPanel } from '../components/ConnectionPanel'

function fmtTs(ts: number) {
  const d = new Date(ts)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function RoomRoute() {
  const { spaceId, roomId } = useParams()
  const decodedSpaceId = spaceId ? decodeURIComponent(spaceId) : ''
  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''

  const { spaces, rooms } = getState()
  const space = spaces.find((s) => s.id === decodedSpaceId)
  const room = rooms.find((r) => r.spaceId === decodedSpaceId && r.id === decodedRoomId)

  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)
  const [err, setErr] = useState<string | null>(null)
  const [rev, setRev] = useState(0)
  const [validEvents, setValidEvents] = useState<EventV1[]>([])
  const [invalidCount, setInvalidCount] = useState(0)
  const [invalidErrors, setInvalidErrors] = useState<string[]>([])

  useEffect(() => {
    const onAppended = (ev: Event) => {
      const ce = ev as CustomEvent<any>
      const d = ce?.detail
      if (d?.v !== 1) return
      if (d.spaceId !== decodedSpaceId || d.roomId !== decodedRoomId) return
      setRev((r) => r + 1)
    }
    window.addEventListener('voxelle-room-event-appended', onAppended)
    return () => window.removeEventListener('voxelle-room-event-appended', onAppended)
  }, [decodedSpaceId, decodedRoomId])

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      const raw = getRoomEvents(decodedSpaceId, decodedRoomId) as unknown as EventV1[]
      const results = await Promise.all(raw.map((e) => validateEvent(e)))
      const ok: EventV1[] = []
      const errs: string[] = []
      for (const r of results) {
        if (r.ok) ok.push(r.value)
        else if (errs.length < 3) errs.push(r.error)
      }
      if (cancelled) return
      setValidEvents(ok)
      setInvalidCount(raw.length - ok.length)
      setInvalidErrors(errs)
    })().catch((e) => {
      if (cancelled) return
      setErr(e instanceof Error ? e.message : String(e))
    })
    return () => {
      cancelled = true
    }
  }, [decodedSpaceId, decodedRoomId, rev])

  const feed = useMemo(() => messagesFromEvents(validEvents), [validEvents])

  if (!space) {
    return <div className="emptyState">Unknown space: {decodedSpaceId || '(missing)'}.</div>
  }
  if (!room) {
    return (
      <div className="emptyState">
        Unknown room for this space: {decodedRoomId || '(missing)'}.
        <div style={{ marginTop: 8 }}>
          <Link to={`/s/${encodeURIComponent(space.id)}`} className="pill accent">
            Back to rooms
          </Link>
        </div>
      </div>
    )
  }

  const spaceOk = space
  const roomOk = room

  async function send() {
    const text = draft.trim()
    if (!text || sending) return
    setSending(true)
    setErr(null)
    try {
      const identity0 = await ensureIdentity()
      const { identity, delegation } = await ensureDelegationForSpace(identity0, spaceOk.id)

      const heads = computeHeads(validEvents)
      const prev = heads.slice(0, 8).sort()
      const ev = await createMsgPostEvent({
        identity,
        delegation,
        spaceId: spaceOk.id,
        roomId: roomOk.id,
        prev,
        text,
      })
      const checked = await validateEvent(ev)
      if (!checked.ok) throw new Error(`refusing to store invalid event: ${checked.error}`)
      appendRoomEvent(spaceOk.id, roomOk.id, checked.value)
      setDraft('')
      setRev((r) => r + 1)
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e))
    } finally {
      setSending(false)
    }
  }

  return (
    <div className="chat">
      <ConnectionPanel spaceId={spaceOk.id} roomId={roomOk.id} />
      <div className="item">
        <div className="itemTop">
          <div className="itemTitle">
            {spaceOk.name} / #{roomOk.name}
          </div>
          <span className="pill">{roomOk.visibility}</span>
        </div>
        <div className="muted" style={{ fontSize: 13 }}>
          {spaceOk.id} • {roomOk.id}
        </div>
      </div>

      <div className="chatFeed" role="log" aria-label="chat feed">
        {invalidCount > 0 ? (
          <div className="emptyState" style={{ marginBottom: 10, borderStyle: 'solid' }}>
            <div style={{ fontWeight: 700, color: 'var(--danger)' }}>
              {invalidCount} invalid event{invalidCount === 1 ? '' : 's'} hidden
            </div>
            {invalidErrors.length > 0 ? (
              <div style={{ marginTop: 6, fontSize: 12 }} className="muted">
                {invalidErrors.join(' • ')}
              </div>
            ) : null}
          </div>
        ) : null}
        {feed.length === 0 ? (
          <div className="emptyState">No messages yet.</div>
        ) : (
          feed.map((m) => (
            <div key={m.id} className="msg">
              <div className="msgHeader">
                <div className="msgAuthor">{m.author}</div>
                <div className="msgTs">{fmtTs(m.ts)}</div>
              </div>
              <div style={{ whiteSpace: 'pre-wrap' }}>{m.text}</div>
              {m.meta?.eventId ? (
                <div className="muted" style={{ marginTop: 6, fontSize: 12 }}>
                  event: {m.meta.eventId}
                </div>
              ) : null}
            </div>
          ))
        )}
      </div>

      <div className="composer" aria-label="message composer">
        <input
          value={draft}
          placeholder="Message…"
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) send()
          }}
        />
        <button onClick={send} disabled={!draft.trim() || sending}>
          {sending ? 'Signing…' : 'Send'}
        </button>
      </div>
      {err ? (
        <div className="muted" style={{ fontSize: 12, color: 'var(--danger)' }}>
          {err}
        </div>
      ) : null}
      <div className="muted" style={{ fontSize: 12 }}>
        Tip: press Ctrl+Enter (or Cmd+Enter) to send.
      </div>
    </div>
  )
}
