import { useEffect, useMemo, useRef, useState } from 'react'
import type { TransportStatus, WebRtcTransportWithSignaling } from '../voxelle/webrtc'
import { createWebRtcTransport } from '../voxelle/webrtc'
import { startRoomSync } from '../voxelle/sync'

function setUrlParam(name: string, value: string | null) {
  const url = new URL(window.location.href)
  if (!value) url.searchParams.delete(name)
  else url.searchParams.set(name, value)
  // avoid triggering full reload, but update bar for share/copy
  window.history.replaceState({}, '', url.toString())
}

function getUrlParam(name: string): string {
  const url = new URL(window.location.href)
  return url.searchParams.get(name) ?? ''
}

async function copyToClipboard(text: string) {
  await navigator.clipboard.writeText(text)
}

export function ConnectionPanel(props: { spaceId: string; roomId: string }) {
  const [stun, setStun] = useState('stun:stun.l.google.com:19302')
  const [offerIn, setOfferIn] = useState('')
  const [answerIn, setAnswerIn] = useState('')
  const [offerOut, setOfferOut] = useState('')
  const [answerOut, setAnswerOut] = useState('')
  const [status, setStatus] = useState<TransportStatus>({ state: 'idle' })
  const [logLines, setLogLines] = useState<string[]>([])
  const [copied, setCopied] = useState<string | null>(null)

  const transportRef = useRef<WebRtcTransportWithSignaling | null>(null)
  const stopSyncRef = useRef<null | (() => void)>(null)
  const autoJoinRan = useRef(false)

  const iceServers = useMemo(() => {
    const urls = stun
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    return urls.length > 0 ? [{ urls }] : []
  }, [stun])

  const log = (s: string) => setLogLines((l) => [...l.slice(-40), `${new Date().toLocaleTimeString()} ${s}`])

  useEffect(() => {
    const offerFromUrl = getUrlParam('offer')
    const answerFromUrl = getUrlParam('answer')
    if (offerFromUrl && !offerIn) setOfferIn(offerFromUrl)
    if (answerFromUrl && !answerIn) setAnswerIn(answerFromUrl)

    return () => {
      stopSyncRef.current?.()
      stopSyncRef.current = null
      transportRef.current?.close()
      transportRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
    setUrlParam('offer', code)
    setUrlParam('answer', null)
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
    setUrlParam('answer', code)
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

  useEffect(() => {
    // If user opened a link with ?offer=, auto-join once.
    if (autoJoinRan.current) return
    const offer = offerIn.trim()
    if (!offer) return
    autoJoinRan.current = true
    join().catch((e) => log(`error: ${e instanceof Error ? e.message : String(e)}`))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [offerIn])

  return (
    <div className="item" style={{ marginBottom: 10 }}>
      <div className="itemTop">
        <div className="itemTitle">Connect (P2P WebRTC)</div>
        <span className="pill accent">{status.state}</span>
      </div>

      <div className="muted" style={{ fontSize: 13 }}>
        Signaling is manual for now: copy/paste offer/answer codes (or share links).
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
        {copied ? <span className="muted" style={{ fontSize: 12 }}>{copied}</span> : null}
      </div>

      {offerOut ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Offer code (send to peer)
          </div>
          <textarea value={offerOut} readOnly style={taStyle} />
          <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
            <button
              onClick={async () => {
                await copyToClipboard(offerOut)
                setCopied('Copied offer code')
                window.setTimeout(() => setCopied(null), 1200)
              }}
            >
              Copy offer
            </button>
            <button
              onClick={async () => {
                const url = new URL(window.location.href)
                url.searchParams.set('offer', offerOut)
                url.searchParams.delete('answer')
                await copyToClipboard(url.toString())
                setCopied('Copied offer link')
                window.setTimeout(() => setCopied(null), 1200)
              }}
            >
              Copy offer link
            </button>
          </div>
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
          <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
            <button
              onClick={async () => {
                await copyToClipboard(answerOut)
                setCopied('Copied answer code')
                window.setTimeout(() => setCopied(null), 1200)
              }}
            >
              Copy answer
            </button>
            <button
              onClick={async () => {
                const url = new URL(window.location.href)
                url.searchParams.set('answer', answerOut)
                await copyToClipboard(url.toString())
                setCopied('Copied answer link')
                window.setTimeout(() => setCopied(null), 1200)
              }}
            >
              Copy answer link
            </button>
          </div>
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
