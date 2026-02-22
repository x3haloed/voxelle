import { Link, useParams } from 'react-router-dom'
import { useEffect, useMemo, useState } from 'react'
import { exportSpaceGenesis, getState, isSpaceOwner, issueInviteFromOwner, onStateChanged, roomsForSpace } from '../voxelle/store'
import { encodeInviteToFragment } from '../voxelle/invite_link'
import { parseInviteRendezvous } from '../voxelle/rfc/invite_bootstrap'

export function SpaceRoute() {
  const { spaceId } = useParams()
  const decoded = spaceId ? decodeURIComponent(spaceId) : ''
  const [rev, setRev] = useState(0)
  useEffect(() => onStateChanged(() => setRev((r) => r + 1)), [])

  const { spaces } = useMemo(() => getState(), [rev])

  const space = spaces.find((s) => s.id === decoded)
  const spaceRooms = useMemo(() => roomsForSpace(decoded), [decoded, rev])
  const owner = useMemo(() => isSpaceOwner(decoded), [decoded, rev])
  const [relayWs, setRelayWs] = useState(() => localStorage.getItem('voxelle.relay.ws') ?? '')
  const [inviteLink, setInviteLink] = useState('')
  const [hostLink, setHostLink] = useState('')
  const [issuing, setIssuing] = useState(false)
  const [err, setErr] = useState<string | null>(null)

  if (!space) {
    return <div className="emptyState">Unknown space: {decoded || '(missing)'}.</div>
  }

  return (
    <div>
      <div className="sectionTitle">Space</div>
      <div className="item">
        <div className="itemTop">
          <div className="itemTitle">{space.name}</div>
          <span className="pill accent">p2p</span>
        </div>
        <div className="muted" style={{ fontSize: 13 }}>
          {space.id}
        </div>
        <div style={{ height: 10 }} />
        <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
          <button
            onClick={async () => {
              const g = exportSpaceGenesis(space.id)
              if (!g) return
              await navigator.clipboard.writeText(JSON.stringify(g))
            }}
          >
            Copy genesis JSON
          </button>
          <span className="muted" style={{ fontSize: 12 }}>
            (dev tool)
          </span>
        </div>
      </div>

      {owner ? (
        <>
          <div className="sectionTitle">Invite</div>
          <div className="item">
            <div className="muted" style={{ fontSize: 13 }}>
              Create an invite capability (signed by this Space Root’s delegated device).
            </div>
            <div style={{ height: 10 }} />
            <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
              <input
                value={relayWs}
                onChange={(e) => setRelayWs(e.target.value)}
                placeholder="Optional relay ws://…/ws (for bootstrap rendezvous)"
                style={{ minWidth: 360 }}
              />
              <button
                className="primary"
                disabled={issuing}
                onClick={async () => {
                  setIssuing(true)
                  setErr(null)
                  try {
                    if (relayWs.trim()) localStorage.setItem('voxelle.relay.ws', relayWs.trim())
                    const inv = await issueInviteFromOwner({
                      spaceId: space.id,
                      spaceNameHint: space.name,
                      relayWsUrl: relayWs.trim() || undefined,
                      expiresInHours: 24 * 7,
                      allowPost: true,
                    })
                    const frag = encodeInviteToFragment(inv)
                    const link = `${window.location.origin}/?${''}${frag}`
                    setInviteLink(link)
                    const rv = parseInviteRendezvous(inv)
                    if (rv?.kind === 'signal-ws') {
                      const qs = new URLSearchParams({ relay: rv.url, sid: rv.sid, role: 'host' })
                      setHostLink(
                        `${window.location.origin}/s/${encodeURIComponent(space.id)}/r/${encodeURIComponent('room:general')}?${qs.toString()}`,
                      )
                    } else {
                      setHostLink('')
                    }
                    await navigator.clipboard.writeText(link)
                  } catch (e) {
                    setErr(e instanceof Error ? e.message : String(e))
                  } finally {
                    setIssuing(false)
                  }
                }}
              >
                {issuing ? 'Issuing…' : 'Create Invite (copy link)'}
              </button>
            </div>
            {inviteLink ? (
              <>
                <div style={{ height: 10 }} />
                <div className="muted" style={{ fontSize: 12 }}>
                  Invite link
                </div>
                <textarea value={inviteLink} readOnly style={{ ...taStyle, minHeight: 70 }} />
                {hostLink ? (
                  <>
                    <div style={{ height: 10 }} />
                    <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
                      <button
                        onClick={async () => {
                          await navigator.clipboard.writeText(hostLink)
                        }}
                      >
                        Copy host link
                      </button>
                      <span className="muted" style={{ fontSize: 12 }}>
                        (open in a second tab/device to host the relay session)
                      </span>
                    </div>
                  </>
                ) : null}
              </>
            ) : null}
            {err ? (
              <div className="muted" style={{ marginTop: 8, fontSize: 12, color: 'var(--danger)' }}>
                {err}
              </div>
            ) : null}
          </div>
        </>
      ) : null}

      <div className="sectionTitle">Rooms</div>
      <div className="list">
        {spaceRooms.map((r) => (
          <Link
            key={`${r.spaceId}:${r.id}`}
            to={`/s/${encodeURIComponent(space.id)}/r/${encodeURIComponent(r.id)}`}
            className="item"
          >
            <div className="itemTop">
              <div className="itemTitle">#{r.name}</div>
              <span className="pill">{r.visibility}</span>
            </div>
            <div className="muted" style={{ fontSize: 13 }}>
              {r.id}
            </div>
          </Link>
        ))}
      </div>
    </div>
  )
}

const taStyle: React.CSSProperties = {
  width: '100%',
  minHeight: 90,
  resize: 'vertical',
  background: 'rgba(22, 33, 74, 0.55)',
  border: '1px solid rgba(255, 255, 255, 0.12)',
  color: 'var(--text)',
  borderRadius: 12,
  padding: '10px 12px',
  outline: 'none',
  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
  fontSize: 12,
}
