import { Link } from 'react-router-dom'
import { getState } from '../voxelle/store'

export function Home() {
  const { spaces } = getState()

  return (
    <div>
      <div className="emptyState">
        <div style={{ fontWeight: 700, color: 'var(--text)' }}>No servers. No accounts.</div>
        <div style={{ marginTop: 6 }}>
          This is the Voxelle UI shell. Next we’ll wire Spaces/Rooms/Events from the RFC.
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

