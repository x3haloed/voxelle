import { Link, useNavigate } from 'react-router-dom'
import { useEffect, useMemo, useState } from 'react'
import { createSpace, getState, joinSpaceFromInvite, onStateChanged } from '../voxelle/store'
import { decodeInviteFromUrl } from '../voxelle/invite_link'
import { parseInviteRendezvous } from '../voxelle/rfc/invite_bootstrap'

export function Home() {
  const nav = useNavigate()
  const [rev, setRev] = useState(0)
  const [name, setName] = useState('')
  const [inviteText, setInviteText] = useState('')
  const [creating, setCreating] = useState(false)
  const [err, setErr] = useState<string | null>(null)
  const [joining, setJoining] = useState(false)

  useEffect(() => onStateChanged(() => setRev((r) => r + 1)), [])

  const { spaces } = useMemo(() => getState(), [rev])

  useEffect(() => {
    // Auto-consume invite links.
    const inv = decodeInviteFromUrl(window.location.href)
    if (!inv) return
    setJoining(true)
    joinSpaceFromInvite(inv)
      .then((s) => {
        const rv = parseInviteRendezvous(inv)
        // clear fragment so refresh doesn't re-join
        window.history.replaceState({}, '', window.location.pathname + window.location.search)
        if (rv?.kind === 'signal-ws') {
          localStorage.setItem('voxelle.relay.ws', rv.url)
          const qs = new URLSearchParams({
            relay: rv.url,
            sid: rv.sid,
            role: 'join',
          })
          nav(`/s/${encodeURIComponent(s.id)}/r/${encodeURIComponent('room:general')}?${qs.toString()}`)
        } else {
          nav(`/s/${encodeURIComponent(s.id)}`)
        }
      })
      .catch((e) => setErr(e instanceof Error ? e.message : String(e)))
      .finally(() => setJoining(false))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div>
      <div className="emptyState">
        <div style={{ fontWeight: 700, color: 'var(--text)' }}>No servers. No accounts.</div>
        <div style={{ marginTop: 6 }}>
          This is the Voxelle UI shell. Next we’ll wire Spaces/Rooms/Events from the RFC.
        </div>
        <div style={{ height: 10 }} />
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="New space name…"
            style={{ minWidth: 260 }}
          />
          <button
            className="primary"
            disabled={creating}
            onClick={async () => {
              setCreating(true)
              setErr(null)
              try {
                const s = await createSpace(name)
                setName('')
                nav(`/s/${encodeURIComponent(s.id)}`)
              } catch (e) {
                setErr(e instanceof Error ? e.message : String(e))
              } finally {
                setCreating(false)
              }
            }}
          >
            {creating ? 'Creating…' : 'Create Space'}
          </button>
          {err ? (
            <span className="muted" style={{ fontSize: 12, color: 'var(--danger)' }}>
              {err}
            </span>
          ) : null}
        </div>
        <div style={{ height: 12 }} />
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          <input
            value={inviteText}
            onChange={(e) => setInviteText(e.target.value)}
            placeholder="Paste invite link (or #invite=...)"
            style={{ minWidth: 320 }}
          />
          <button
            disabled={joining || !inviteText.trim()}
            onClick={async () => {
              setJoining(true)
              setErr(null)
              try {
                const inv = decodeInviteFromUrl(inviteText.trim())
                if (!inv) throw new Error('could not parse invite')
                const s = await joinSpaceFromInvite(inv)
                setInviteText('')
                const rv = parseInviteRendezvous(inv)
                if (rv?.kind === 'signal-ws') {
                  localStorage.setItem('voxelle.relay.ws', rv.url)
                  const qs = new URLSearchParams({
                    relay: rv.url,
                    sid: rv.sid,
                    role: 'join',
                  })
                  nav(`/s/${encodeURIComponent(s.id)}/r/${encodeURIComponent('room:general')}?${qs.toString()}`)
                } else {
                  nav(`/s/${encodeURIComponent(s.id)}`)
                }
              } catch (e) {
                setErr(e instanceof Error ? e.message : String(e))
              } finally {
                setJoining(false)
              }
            }}
          >
            {joining ? 'Joining…' : 'Join Space'}
          </button>
        </div>
      </div>

      <div className="sectionTitle">Spaces</div>
      <div className="list">
        {spaces.map((s) => (
          <Link key={s.id} to={`/s/${encodeURIComponent(s.id)}`} className="item">
            <div className="itemTop">
              <div className="itemTitle">{s.name}</div>
              <span className="pill">invite-only</span>
            </div>
            <div className="muted" style={{ fontSize: 13 }}>
              {s.id}
            </div>
          </Link>
        ))}
      </div>
    </div>
  )
}
