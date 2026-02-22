import { useEffect, useMemo, useRef, useState } from 'react'
import type { TransportStatus, WebRtcTransportWithSignaling } from '../voxelle/webrtc'
import { createWebRtcTransport } from '../voxelle/webrtc'
import { startRoomSync } from '../voxelle/sync'

export function ConnectionPanel(props: { spaceId: string; roomId: string }) {
  const [stun, setStun] = useState('stun:stun.l.google.com:19302')
  const [offerIn, setOfferIn] = useState('')
  const [answerIn, setAnswerIn] = useState('')
  const [offerOut, setOfferOut] = useState('')
  const [answerOut, setAnswerOut] = useState('')
  const [status, setStatus] = useState<TransportStatus>({ state: 'idle' })
  const [logLines, setLogLines] = useState<string[]>([])

  const transportRef = useRef<WebRtcTransportWithSignaling | null>(null)
  const stopSyncRef = useRef<null | (() => void)>(null)

  const iceServers = useMemo(() => {
    const urls = stun
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    return urls.length > 0 ? [{ urls }] : []
  }, [stun])

  const log = (s: string) => setLogLines((l) => [...l.slice(-40), `${new Date().toLocaleTimeString()} ${s}`])

  useEffect(() => {
    return () => {
      stopSyncRef.current?.()
      stopSyncRef.current = null
      transportRef.current?.close()
      transportRef.current = null
    }
  }, [])

  async function ensureTransport(): Promise<WebRtcTransportWithSignaling> {
    if (transportRef.current) return transportRef.current
    const t = createWebRtcTransport({ iceServers }) as WebRtcTransportWithSignaling
    transportRef.current = t
    t.onState((s) => setStatus(s))
    return t
  }

  function startSyncIfConnected(t: WebRtcTransportWithSignaling) {
    if (stopSyncRef.current) return
    if (t.getState().state !== 'connected') return
    stopSyncRef.current = startRoomSync({
      transport: t,
      spaceId: props.spaceId,
      roomId: props.roomId,
      onLog: (line) => log(`sync: ${line}`),
    })
    log('sync: started')
  }

  async function host() {
    setOfferOut('')
    setAnswerOut('')
    setOfferIn('')
    setAnswerIn('')
    setLogLines([])
    const t = await ensureTransport()
    const code = await t.startOffer()
    setOfferOut(code)
    log('local: offer created (share with peer)')
  }

  async function join() {
    setOfferOut('')
    setAnswerOut('')
    setAnswerIn('')
    setLogLines([])
    const t = await ensureTransport()
    const code = await t.acceptOfferAndMakeAnswer(offerIn.trim())
    setAnswerOut(code)
    log('local: answer created (send back to host)')
  }

  async function acceptAnswer() {
    const t = await ensureTransport()
    await t.acceptAnswer(answerIn.trim())
    log('local: answer accepted; waiting for connect')
  }

  async function disconnect() {
    stopSyncRef.current?.()
    stopSyncRef.current = null
    transportRef.current?.close()
    transportRef.current = null
    setStatus({ state: 'closed' })
    log('local: closed')
  }

  useEffect(() => {
    const t = transportRef.current
    if (!t) return
    if (status.state === 'connected') startSyncIfConnected(t)
  }, [status.state])

  return (
    <div className="item" style={{ marginBottom: 10 }}>
      <div className="itemTop">
        <div className="itemTitle">Connect (P2P WebRTC)</div>
        <span className="pill accent">{status.state}</span>
      </div>

      <div className="muted" style={{ fontSize: 13 }}>
        Signaling is manual for now: copy/paste offer/answer codes.
      </div>

      <div style={{ height: 10 }} />

      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        <span className="pill">STUN</span>
        <input
          value={stun}
          onChange={(e) => setStun(e.target.value)}
          placeholder="stun:host:port (comma-separated)"
          style={{ minWidth: 360 }}
        />
        <button onClick={host}>Host</button>
        <button onClick={disconnect}>Disconnect</button>
      </div>

      {offerOut ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Offer code (send to peer)
          </div>
          <textarea value={offerOut} readOnly style={taStyle} />
          <div className="row" style={{ gap: 8 }}>
            <input
              value={answerIn}
              onChange={(e) => setAnswerIn(e.target.value)}
              placeholder="Paste answer code here"
              style={{ flex: 1 }}
            />
            <button onClick={acceptAnswer} disabled={!answerIn.trim()}>
              Accept answer
            </button>
          </div>
        </>
      ) : null}

      {!offerOut ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Join as peer (paste offer)
          </div>
          <div className="row" style={{ gap: 8 }}>
            <input
              value={offerIn}
              onChange={(e) => setOfferIn(e.target.value)}
              placeholder="Paste offer code here"
              style={{ flex: 1 }}
            />
            <button onClick={join} disabled={!offerIn.trim()}>
              Create answer
            </button>
          </div>
        </>
      ) : null}

      {answerOut ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Answer code (send back to host)
          </div>
          <textarea value={answerOut} readOnly style={taStyle} />
        </>
      ) : null}

      {logLines.length > 0 ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Sync log
          </div>
          <div className="chatFeed" style={{ maxHeight: 160, padding: 8 }}>
            {logLines.map((l, i) => (
              <div key={i} className="muted" style={{ fontSize: 12, marginBottom: 4 }}>
                {l}
              </div>
            ))}
          </div>
        </>
      ) : null}
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

