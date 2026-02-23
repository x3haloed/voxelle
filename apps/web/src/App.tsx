import { Link, Route, Routes, useParams } from 'react-router-dom'
import { useEffect, useMemo, useState } from 'react'
import './index.css'
import { Topbar } from './components/Topbar'
import { Home } from './routes/Home'
import { RoomRoute } from './routes/Room'
import { SpaceRoute } from './routes/Space'
import { getState, hydrateRoomEventsFromIndexedDb, onStateChanged } from './voxelle/store'

function App() {
  const { spaceId, roomId } = useParams()
  const decodedSpaceId = spaceId ? decodeURIComponent(spaceId) : ''
  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''

  const [rev, setRev] = useState(0)
  useEffect(() => onStateChanged(() => setRev((r) => r + 1)), [])
  useEffect(() => {
    hydrateRoomEventsFromIndexedDb()
      .then((n) => {
        if (n > 0) setRev((r) => r + 1)
      })
      .catch(() => {})
  }, [])

  const { rooms, spaces } = useMemo(() => getState(), [rev])
  const activeSpace = spaces.find((s) => s.id === decodedSpaceId)
  const activeRooms = activeSpace ? rooms.filter((r) => r.spaceId === activeSpace.id) : []
  const activeRoom = activeRooms.find((r) => r.id === decodedRoomId)

  return (
    <div className="appShell">
      <Topbar />
      <div className="main">
        <aside className="sidebar" aria-label="sidebar">
          <div className="sectionTitle">Navigation</div>
          <div className="list">
            <Link to="/" className="item">
              <div className="itemTitle">Spaces</div>
              <div className="muted" style={{ fontSize: 13 }}>
                Choose a space to enter.
              </div>
            </Link>
          </div>

          {activeSpace ? (
            <>
              <div className="sectionTitle">Rooms</div>
              <div className="list">
                {activeRooms.map((r) => (
                  <Link
                    key={r.id}
                    to={`/s/${encodeURIComponent(activeSpace.id)}/r/${encodeURIComponent(r.id)}`}
                    className="item"
                    aria-current={activeRoom?.id === r.id ? 'page' : undefined}
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
            </>
          ) : (
            <div className="emptyState" style={{ marginTop: 10 }}>
              Pick a Space to see its Rooms.
            </div>
          )}
        </aside>

        <main className="content" aria-label="content">
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/s/:spaceId" element={<SpaceRoute />} />
            <Route path="/s/:spaceId/r/:roomId" element={<RoomRoute />} />
            <Route path="*" element={<div className="emptyState">Not found.</div>} />
          </Routes>
        </main>
      </div>
    </div>
  )
}

export default App
