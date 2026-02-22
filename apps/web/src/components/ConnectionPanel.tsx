import { useEffect, useMemo, useRef, useState } from 'react'
import type { TransportStatus, WebRtcTransportWithSignaling } from '../voxelle/webrtc'
import { createWebRtcTransport } from '../voxelle/webrtc'
import { startRoomSync, type SyncStats } from '../voxelle/sync'
import { createSignalClient, newSessionId, type SignalClient, type SignalState } from '../voxelle/signal_ws'
import { getRoomEvents, getRoomHeads } from '../voxelle/store'

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
  const [mode, setMode] = useState<'manual' | 'relay'>('manual')
  const [stun, setStun] = useState('stun:stun.l.google.com:19302')
  const [relayUrl, setRelayUrl] = useState(() => localStorage.getItem('voxelle.relay.ws') ?? '')
  const [sid, setSid] = useState(() => getUrlParam('sid'))
  const [offerIn, setOfferIn] = useState('')
  const [answerIn, setAnswerIn] = useState('')
  const [offerOut, setOfferOut] = useState('')
  const [answerOut, setAnswerOut] = useState('')
  const [status, setStatus] = useState<TransportStatus>({ state: 'idle' })
  const [logLines, setLogLines] = useState<string[]>([])
  const [syncStats, setSyncStats] = useState<SyncStats>({ phase: 'starting' })
  const [localRev, setLocalRev] = useState(0)
  const [copied, setCopied] = useState<string | null>(null)

  const transportRef = useRef<WebRtcTransportWithSignaling | null>(null)
  const signalRef = useRef<SignalClient | null>(null)
  const stopSyncRef = useRef<null | (() => void)>(null)
  const autoJoinRan = useRef(false)
  const autoHostRan = useRef(false)
  const autoAcceptAnswerRan = useRef(false)
  const modeRef = useRef(mode)
  const sidRef = useRef(sid)
  const relayUrlRef = useRef(relayUrl)
  const answerOutRef = useRef(answerOut)

  const iceServers = useMemo(() => {
    const urls = stun
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    return urls.length > 0 ? [{ urls }] : []
  }, [stun])

  const log = (s: string) => setLogLines((l) => [...l.slice(-40), `${new Date().toLocaleTimeString()} ${s}`])

  const localEventCount = useMemo(() => getRoomEvents(props.spaceId, props.roomId).length, [props.spaceId, props.roomId, localRev])
  const localHeadsCount = useMemo(() => getRoomHeads(props.spaceId, props.roomId).length, [props.spaceId, props.roomId, localRev])

  useEffect(() => {
    modeRef.current = mode
  }, [mode])
  useEffect(() => {
    sidRef.current = sid
  }, [sid])
  useEffect(() => {
    relayUrlRef.current = relayUrl
  }, [relayUrl])
  useEffect(() => {
    answerOutRef.current = answerOut
  }, [answerOut])

  useEffect(() => {
    const offerFromUrl = getUrlParam('offer')
    const answerFromUrl = getUrlParam('answer')
    const relayFromUrl = getUrlParam('relay')
    const sidFromUrl = getUrlParam('sid')
    const roleFromUrl = getUrlParam('role')
    if (offerFromUrl && !offerIn) setOfferIn(offerFromUrl)
    if (answerFromUrl && !answerIn) setAnswerIn(answerFromUrl)
    if (relayFromUrl) {
      setMode('relay')
      setRelayUrl(relayFromUrl)
    }
    if (sidFromUrl) setSid(sidFromUrl)
    if (roleFromUrl === 'join' || roleFromUrl === 'host') setMode('relay')

    return () => {
      stopSyncRef.current?.()
      stopSyncRef.current = null
      transportRef.current?.close()
      transportRef.current = null
      signalRef.current?.close()
      signalRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    localStorage.setItem('voxelle.relay.ws', relayUrl)
  }, [relayUrl])

  useEffect(() => {
    const onAppended = (ev: Event) => {
      const ce = ev as CustomEvent<any>
      const d = ce?.detail
      if (d?.v !== 1) return
      if (d.spaceId !== props.spaceId || d.roomId !== props.roomId) return
      setLocalRev((r) => r + 1)
    }
    window.addEventListener('voxelle-room-event-appended', onAppended)
    return () => window.removeEventListener('voxelle-room-event-appended', onAppended)
  }, [props.spaceId, props.roomId])

  async function ensureTransport(): Promise<WebRtcTransportWithSignaling> {
    if (transportRef.current) return transportRef.current
    const t = createWebRtcTransport({ iceServers }) as WebRtcTransportWithSignaling
    transportRef.current = t
    t.onState((s) => setStatus(s))
    return t
  }

  function resetAll() {
    stopSyncRef.current?.()
    stopSyncRef.current = null
    transportRef.current?.close()
    transportRef.current = null
    signalRef.current?.close()
    signalRef.current = null
    autoAcceptAnswerRan.current = false
    setSyncStats({ phase: 'starting' })
  }

  function ensureSignal(): SignalClient {
    if (signalRef.current) return signalRef.current
    const url = relayUrlRef.current.trim()
    if (!url) throw new Error('missing relay URL')
    const c = createSignalClient(url)
    signalRef.current = c
    c.onError((e) => log(`relay: ${e}`))
    c.onState((s) => onRelayState(s))
    return c
  }

  async function onRelayState(s: SignalState) {
    const currentSid = sidRef.current
    const currentMode = modeRef.current
    if (!s.sid || !currentSid || s.sid !== currentSid) return

    if (s.offer && currentMode === 'relay') {
      // If this tab is in join role, create answer once we see offer.
      const role = getUrlParam('role')
      if (role === 'join' && !answerOutRef.current) {
        try {
          const t = await ensureTransport()
          const code = await t.acceptOfferAndMakeAnswer(s.offer)
          setAnswerOut(code)
          ensureSignal().setAnswer(s.sid, code)
          setUrlParam('answer', code)
          log('relay: posted answer')
        } catch (e) {
          log(`relay: failed to answer: ${e instanceof Error ? e.message : String(e)}`)
        }
      }
    }
    if (s.answer && currentMode === 'relay') {
      // If this tab is host role, accept answer once.
      const role = getUrlParam('role')
      if (role === 'host' && !autoAcceptAnswerRan.current) {
        autoAcceptAnswerRan.current = true
        try {
          setAnswerIn(s.answer)
          const t = await ensureTransport()
          await t.acceptAnswer(s.answer)
          log('relay: accepted answer')
        } catch (e) {
          log(`relay: failed to accept answer: ${e instanceof Error ? e.message : String(e)}`)
        }
      }
    }
  }

  function startSyncIfConnected(t: WebRtcTransportWithSignaling) {
    if (stopSyncRef.current) return
    if (t.getState().state !== 'connected') return
    stopSyncRef.current = startRoomSync({
      transport: t,
      spaceId: props.spaceId,
      roomId: props.roomId,
      onLog: (line) => log(`sync: ${line}`),
      onStats: (st) => setSyncStats(st),
    })
    log('sync: started')
  }

  async function host() {
    resetAll()
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
    setUrlParam('relay', null)
    setUrlParam('sid', null)
    setUrlParam('role', null)
    log('local: offer created (share with peer)')
  }

  async function join() {
    resetAll()
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

  async function hostRelay() {
    resetAll()
    setOfferOut('')
    setAnswerOut('')
    setOfferIn('')
    setAnswerIn('')
    setLogLines([])

    const existingSid = sidRef.current.trim()
    const nextSid = existingSid || newSessionId()
    setSid(nextSid)
    sidRef.current = nextSid
    setUrlParam('relay', relayUrlRef.current)
    setUrlParam('sid', nextSid)
    setUrlParam('role', 'host')
    setUrlParam('offer', null)
    setUrlParam('answer', null)

    const signal = ensureSignal()
    signal.join(nextSid)

    const t = await ensureTransport()
    const offerCode = await t.startOffer()
    setOfferOut(offerCode)
    signal.setOffer(nextSid, offerCode)
    log('relay: posted offer (share link with peer)')
  }

  async function joinRelay() {
    resetAll()
    setOfferOut('')
    setAnswerOut('')
    setAnswerIn('')
    setLogLines([])

    const sidTrim = sid.trim()
    if (!sidTrim) {
      log('relay: missing sid')
      return
    }
    setUrlParam('relay', relayUrlRef.current)
    setUrlParam('sid', sidTrim)
    setUrlParam('role', 'join')
    setUrlParam('offer', null)

    const signal = ensureSignal()
    signal.join(sidTrim)
    signal.getState(sidTrim)
    log('relay: joined; waiting for offer')
  }

  async function acceptAnswer() {
    const t = await ensureTransport()
    await t.acceptAnswer(answerIn.trim())
    log('local: answer accepted; waiting for connect')
  }

  async function disconnect() {
    resetAll()
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

  useEffect(() => {
    // If user opened a link with ?relay= & ?sid= & role=join, auto-join relay once.
    if (autoJoinRan.current) return
    const relay = getUrlParam('relay')
    const sid0 = getUrlParam('sid')
    const role = getUrlParam('role')
    if (!relay || !sid0 || role !== 'join') return
    autoJoinRan.current = true
    setMode('relay')
    setRelayUrl(relay)
    setSid(sid0)
    relayUrlRef.current = relay
    sidRef.current = sid0
    joinRelay().catch((e) => log(`error: ${e instanceof Error ? e.message : String(e)}`))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sid, relayUrl])

  useEffect(() => {
    // If user opened a link with ?relay= & ?sid= & role=host, auto-host relay once.
    if (autoHostRan.current) return
    const relay = getUrlParam('relay')
    const sid0 = getUrlParam('sid')
    const role = getUrlParam('role')
    if (!relay || !sid0 || role !== 'host') return
    autoHostRan.current = true
    setMode('relay')
    setRelayUrl(relay)
    setSid(sid0)
    relayUrlRef.current = relay
    sidRef.current = sid0
    hostRelay().catch((e) => log(`error: ${e instanceof Error ? e.message : String(e)}`))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sid, relayUrl])

  return (
    <div className="item" style={{ marginBottom: 10 }}>
      <div className="itemTop">
        <div className="itemTitle">Connect (P2P WebRTC)</div>
        <span className="pill accent">{status.state}</span>
      </div>

      <div className="muted" style={{ fontSize: 13 }}>
        Signaling modes: manual (copy/paste) or optional untrusted relay (WebSocket).
      </div>

      <div style={{ height: 10 }} />

      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        <span className="pill">Local events</span>
        <span className="muted" style={{ fontSize: 12 }}>
          {localEventCount}
        </span>
        <span className="pill">Local heads</span>
        <span className="muted" style={{ fontSize: 12 }}>
          {localHeadsCount}
        </span>
        <span className="pill">Peer heads</span>
        <span className="muted" style={{ fontSize: 12 }}>
          {syncStats.lastPeerHeadsCount ?? '—'}
        </span>
        <span className="pill">Last want</span>
        <span className="muted" style={{ fontSize: 12 }}>
          {syncStats.lastWantCount ?? '—'}
        </span>
        <span className="pill">Last have</span>
        <span className="muted" style={{ fontSize: 12 }}>
          {syncStats.lastHaveAccepted !== undefined && syncStats.lastHaveReceived !== undefined
            ? `${syncStats.lastHaveAccepted}/${syncStats.lastHaveReceived}`
            : '—'}
        </span>
      </div>

      <div style={{ height: 10 }} />

      <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
        <span className="pill">Mode</span>
        <select value={mode} onChange={(e) => setMode(e.target.value as any)}>
          <option value="manual">manual</option>
          <option value="relay">relay</option>
        </select>
        <span className="pill">STUN</span>
        <input
          value={stun}
          onChange={(e) => setStun(e.target.value)}
          placeholder="stun:host:port (comma-separated)"
          style={{ minWidth: 360 }}
        />
        {mode === 'manual' ? (
          <button onClick={host}>Host</button>
        ) : (
          <button onClick={hostRelay} disabled={!relayUrl.trim()}>
            Host (relay)
          </button>
        )}
        <button onClick={disconnect}>Disconnect</button>
        {copied ? <span className="muted" style={{ fontSize: 12 }}>{copied}</span> : null}
      </div>

      {mode === 'relay' ? (
        <>
          <div style={{ height: 10 }} />
          <div className="row" style={{ gap: 8, flexWrap: 'wrap' }}>
            <span className="pill">Relay</span>
            <input
              value={relayUrl}
              onChange={(e) => setRelayUrl(e.target.value)}
              placeholder="ws://host:port/ws"
              style={{ minWidth: 360 }}
            />
            <span className="pill">sid</span>
            <input value={sid} onChange={(e) => setSid(e.target.value)} placeholder="session id" style={{ minWidth: 220 }} />
            <button onClick={joinRelay} disabled={!sid.trim() || !relayUrl.trim()}>
              Join (relay)
            </button>
          </div>
        </>
      ) : null}

      {offerOut ? (
        <>
          <div style={{ height: 10 }} />
          <div className="muted" style={{ fontSize: 12 }}>
            Offer code {mode === 'relay' ? '(relay also has it)' : '(send to peer)'}
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
                if (mode === 'manual') {
                  url.searchParams.set('offer', offerOut)
                  url.searchParams.delete('answer')
                  url.searchParams.delete('relay')
                  url.searchParams.delete('sid')
                  url.searchParams.delete('role')
                } else {
                  url.searchParams.set('relay', relayUrl)
                  url.searchParams.set('sid', sid)
                  url.searchParams.set('role', 'join')
                  url.searchParams.delete('offer')
                }
                await copyToClipboard(url.toString())
                setCopied(mode === 'manual' ? 'Copied offer link' : 'Copied join link')
                window.setTimeout(() => setCopied(null), 1200)
              }}
            >
              {mode === 'manual' ? 'Copy offer link' : 'Copy join link'}
            </button>
          </div>
          {mode === 'manual' ? (
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
          ) : (
            <div className="muted" style={{ fontSize: 12 }}>
              Waiting for peer to join and post an answer…
            </div>
          )}
        </>
      ) : null}

      {!offerOut && mode === 'manual' ? (
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
