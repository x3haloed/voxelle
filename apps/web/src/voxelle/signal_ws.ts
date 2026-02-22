export type SignalState = {
  sid: string
  offer?: string
  answer?: string
}

export type SignalClient = {
  close: () => void
  join: (sid: string) => void
  setOffer: (sid: string, offer: string) => void
  setAnswer: (sid: string, answer: string) => void
  getState: (sid: string) => void
  onState: (fn: (s: SignalState) => void) => () => void
  onError: (fn: (e: string) => void) => () => void
}

type ServerMsg =
  | { t: 'hello'; v: 1 }
  | { t: 'state'; v: 1; sid: string; offer?: string | null; answer?: string | null }
  | { t: 'error'; v: 1; error: string }

export function createSignalClient(relayWsUrl: string): SignalClient {
  const ws = new WebSocket(relayWsUrl)
  const stateSubs = new Set<(s: SignalState) => void>()
  const errSubs = new Set<(e: string) => void>()

  const emitErr = (e: string) => {
    for (const fn of errSubs) fn(e)
  }
  const emitState = (s: SignalState) => {
    for (const fn of stateSubs) fn(s)
  }

  ws.addEventListener('message', (ev) => {
    try {
      const msg = JSON.parse(String(ev.data)) as ServerMsg
      if (!msg || msg.v !== 1) return
      if (msg.t === 'error') {
        emitErr(msg.error || 'relay error')
        return
      }
      if (msg.t === 'state') {
        emitState({
          sid: msg.sid,
          offer: msg.offer ?? undefined,
          answer: msg.answer ?? undefined,
        })
      }
    } catch (e) {
      emitErr(e instanceof Error ? e.message : String(e))
    }
  })
  ws.addEventListener('error', () => emitErr('websocket error'))
  ws.addEventListener('close', () => emitErr('websocket closed'))

  const send = (obj: unknown) => {
    const data = JSON.stringify(obj)
    if (ws.readyState === WebSocket.OPEN) ws.send(data)
    else {
      const onOpen = () => {
        ws.removeEventListener('open', onOpen)
        if (ws.readyState === WebSocket.OPEN) ws.send(data)
      }
      ws.addEventListener('open', onOpen)
    }
  }

  return {
    close: () => ws.close(),
    join: (sid: string) => send({ t: 'join', v: 1, sid }),
    setOffer: (sid: string, offer: string) => send({ t: 'set_offer', v: 1, sid, offer }),
    setAnswer: (sid: string, answer: string) => send({ t: 'set_answer', v: 1, sid, answer }),
    getState: (sid: string) => send({ t: 'get_state', v: 1, sid }),
    onState: (fn) => {
      stateSubs.add(fn)
      return () => stateSubs.delete(fn)
    },
    onError: (fn) => {
      errSubs.add(fn)
      return () => errSubs.delete(fn)
    },
  }
}

export function newSessionId(): string {
  const bytes = new Uint8Array(16)
  crypto.getRandomValues(bytes)
  return [...bytes].map((b) => b.toString(16).padStart(2, '0')).join('')
}

