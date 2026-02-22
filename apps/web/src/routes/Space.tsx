import { Link, useParams } from 'react-router-dom'
import { useEffect, useMemo, useState } from 'react'
import { exportSpaceGenesis, getState, onStateChanged, roomsForSpace } from '../voxelle/store'

export function SpaceRoute() {
  const { spaceId } = useParams()
  const decoded = spaceId ? decodeURIComponent(spaceId) : ''
  const [rev, setRev] = useState(0)
  useEffect(() => onStateChanged(() => setRev((r) => r + 1)), [])

  const { spaces } = useMemo(() => getState(), [rev])

  const space = spaces.find((s) => s.id === decoded)
  const spaceRooms = useMemo(() => roomsForSpace(decoded), [decoded, rev])

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
