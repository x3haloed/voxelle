import { Link, useLocation, useParams } from 'react-router-dom'

function tinyId(id: string): string {
  if (!id) return ''
  if (id.length <= 24) return id
  return `${id.slice(0, 12)}…${id.slice(-8)}`
}

export function Topbar() {
  const location = useLocation()
  const { spaceId, roomId } = useParams()

  return (
    <header className="topbar">
      <Link to="/" className="brand">
        Voxelle
      </Link>
      <div className="crumbs" aria-label="breadcrumbs">
        <span className="pill">serverless</span>
        {spaceId ? (
          <>
            <span className="muted">/</span>
            <Link to={`/s/${encodeURIComponent(spaceId)}`} className="pill accent">
              {tinyId(spaceId)}
            </Link>
          </>
        ) : null}
        {roomId ? (
          <>
            <span className="muted">/</span>
            <span className="pill accent">{tinyId(roomId)}</span>
          </>
        ) : null}
        <span className="muted" style={{ marginLeft: 8 }}>
          {location.pathname}
        </span>
      </div>
      <div style={{ marginLeft: 'auto' }} className="pill">
        mock UI
      </div>
    </header>
  )
}

