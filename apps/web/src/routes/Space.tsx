import { Link, useParams } from 'react-router-dom'
import { getState } from '../voxelle/store'

export function SpaceRoute() {
  const { spaceId } = useParams()
  const decoded = spaceId ? decodeURIComponent(spaceId) : ''
  const { spaces, rooms } = getState()

  const space = spaces.find((s) => s.id === decoded)
  const spaceRooms = rooms.filter((r) => r.spaceId === decoded)

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
      </div>

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

