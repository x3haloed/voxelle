import { useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { getState, getRoomEvents, appendRoomEvent } from '../voxelle/store'
import { messagesFromEvents } from '../voxelle/events'
import { ensureIdentity, ensureDelegationForSpace, createMsgPostEvent } from '../voxelle/rfc/signing'

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

  const feed = useMemo(
    () => {
      const evs = getRoomEvents(decodedSpaceId, decodedRoomId)
      return messagesFromEvents(evs)
    },
    [decodedSpaceId, decodedRoomId],
  )

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

      const existing = getRoomEvents(spaceOk.id, roomOk.id)
      const prev = existing.length > 0 ? [existing[existing.length - 1]!.event_id] : []
      const ev = await createMsgPostEvent({
        identity,
        delegation,
        spaceId: spaceOk.id,
        roomId: roomOk.id,
        prev,
        text,
      })
      appendRoomEvent(spaceOk.id, roomOk.id, ev)
      setDraft('')
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e))
    } finally {
      setSending(false)
    }
  }

  return (
    <div className="chat">
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
