import { Link, useNavigate } from 'react-router-dom'
import { useEffect, useMemo, useState } from 'react'
import { createSpace, getState, onStateChanged } from '../voxelle/store'

export function Home() {
  const nav = useNavigate()
  const [rev, setRev] = useState(0)
  const [name, setName] = useState('')
  const [creating, setCreating] = useState(false)
  const [err, setErr] = useState<string | null>(null)

  useEffect(() => onStateChanged(() => setRev((r) => r + 1)), [])

  const { spaces } = useMemo(() => getState(), [rev])

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
