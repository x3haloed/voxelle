export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue }

export type TransportStatus =
  | { state: 'idle' }
  | { state: 'gathering' }
  | { state: 'awaiting_answer'; offerCode: string }
  | { state: 'awaiting_offer' }
  | { state: 'awaiting_offer_paste' }
  | { state: 'awaiting_offer_ack'; answerCode: string }
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'closed' }
  | { state: 'error'; error: string }

export type WebRtcOptions = {
  iceServers?: RTCIceServer[]
  maxMessageBytes?: number
}

export type WebRtcTransport = {
  close: () => void
  send: (msg: JsonValue) => void
  getState: () => TransportStatus
  onState: (fn: (s: TransportStatus) => void) => () => void
  onMessage: (fn: (m: JsonValue) => void) => () => void
}

type WireDescription = {
  sdp: string
  type: RTCSdpType
}

type OfferCodeV1 = {
  v: 1
  offer: WireDescription
}

type AnswerCodeV1 = {
  v: 1
  answer: WireDescription
}

function base64UrlEncodeString(s: string): string {
  const bytes = new TextEncoder().encode(s)
  let bin = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    const slice = bytes.subarray(i, i + chunk)
    bin += String.fromCharCode(...slice)
  }
  const b64 = btoa(bin)
  return b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}

function base64UrlDecodeToString(s: string): string {
  let b64 = s.replace(/-/g, '+').replace(/_/g, '/')
  while (b64.length % 4 !== 0) b64 += '='
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return new TextDecoder().decode(bytes)
}

function safeJsonParse(s: string): unknown {
  return JSON.parse(s)
}

function waitIceGatheringComplete(pc: RTCPeerConnection, timeoutMs = 30_000): Promise<void> {
  return new Promise((resolve, reject) => {
    if (pc.iceGatheringState === 'complete') return resolve()
    const t = window.setTimeout(() => {
      cleanup()
      reject(new Error('ICE gathering timed out'))
    }, timeoutMs)
    const onState = () => {
      if (pc.iceGatheringState === 'complete') {
        cleanup()
        resolve()
      }
    }
    const cleanup = () => {
      window.clearTimeout(t)
      pc.removeEventListener('icegatheringstatechange', onState)
    }
    pc.addEventListener('icegatheringstatechange', onState)
  })
}

function createPeerConnection(opts: WebRtcOptions): RTCPeerConnection {
  return new RTCPeerConnection({
    iceServers: opts.iceServers ?? [{ urls: ['stun:stun.l.google.com:19302'] }],
  })
}

export function createWebRtcTransport(opts: WebRtcOptions = {}): WebRtcTransport {
  const maxMessageBytes = opts.maxMessageBytes ?? 256 * 1024
  let pc: RTCPeerConnection | null = null
  let dc: RTCDataChannel | null = null

  let state: TransportStatus = { state: 'idle' }
  const stateSubs = new Set<(s: TransportStatus) => void>()
  const msgSubs = new Set<(m: JsonValue) => void>()

  const emitState = (s: TransportStatus) => {
    state = s
    for (const fn of stateSubs) fn(s)
  }
  const emitMsg = (m: JsonValue) => {
    for (const fn of msgSubs) fn(m)
  }

  const close = () => {
    try {
      dc?.close()
    } catch {}
    try {
      pc?.close()
    } catch {}
    dc = null
    pc = null
    emitState({ state: 'closed' })
  }

  const ensurePc = () => {
    if (pc) return pc
    pc = createPeerConnection(opts)
    pc.addEventListener('connectionstatechange', () => {
      if (!pc) return
      if (pc.connectionState === 'connected') emitState({ state: 'connected' })
      if (pc.connectionState === 'failed') emitState({ state: 'error', error: 'WebRTC connection failed' })
      if (pc.connectionState === 'closed') emitState({ state: 'closed' })
      if (pc.connectionState === 'disconnected') emitState({ state: 'connecting' })
    })
    pc.addEventListener('datachannel', (ev) => {
      dc = ev.channel
      wireDataChannel(dc)
    })
    return pc
  }

  const wireDataChannel = (channel: RTCDataChannel) => {
    channel.binaryType = 'arraybuffer'
    channel.addEventListener('open', () => emitState({ state: 'connected' }))
    channel.addEventListener('close', () => emitState({ state: 'closed' }))
    channel.addEventListener('message', (ev) => {
      try {
        const data = typeof ev.data === 'string' ? ev.data : new TextDecoder().decode(new Uint8Array(ev.data))
        if (data.length > maxMessageBytes) throw new Error('message too large')
        const msg = safeJsonParse(data) as JsonValue
        emitMsg(msg)
      } catch (e) {
        emitState({ state: 'error', error: e instanceof Error ? e.message : String(e) })
      }
    })
  }

  const send = (msg: JsonValue) => {
    if (!dc || dc.readyState !== 'open') throw new Error('data channel not open')
    const s = JSON.stringify(msg)
    if (s.length > maxMessageBytes) throw new Error('message too large')
    dc.send(s)
  }

  const startOffer = async (): Promise<string> => {
    try {
      emitState({ state: 'gathering' })
      const pc0 = ensurePc()
      dc = pc0.createDataChannel('voxelle', { ordered: true })
      wireDataChannel(dc)
      const offer = await pc0.createOffer()
      await pc0.setLocalDescription(offer)
      await waitIceGatheringComplete(pc0)
      const ld = pc0.localDescription
      if (!ld?.sdp || !ld.type) throw new Error('missing localDescription')
      const code: OfferCodeV1 = { v: 1, offer: { sdp: ld.sdp, type: ld.type } }
      const offerCode = base64UrlEncodeString(JSON.stringify(code))
      emitState({ state: 'awaiting_answer', offerCode })
      return offerCode
    } catch (e) {
      emitState({ state: 'error', error: e instanceof Error ? e.message : String(e) })
      throw e
    }
  }

  const acceptOfferAndMakeAnswer = async (offerCode: string): Promise<string> => {
    try {
      emitState({ state: 'gathering' })
      const pc0 = ensurePc()
      const decoded = safeJsonParse(base64UrlDecodeToString(offerCode)) as OfferCodeV1
      if (!decoded || decoded.v !== 1) throw new Error('invalid offer code')
      const offer = decoded.offer
      if (!offer?.sdp || !offer?.type) throw new Error('invalid offer contents')
      await pc0.setRemoteDescription(offer)
      const answer = await pc0.createAnswer()
      await pc0.setLocalDescription(answer)
      await waitIceGatheringComplete(pc0)
      const ld = pc0.localDescription
      if (!ld?.sdp || !ld.type) throw new Error('missing localDescription')
      const code: AnswerCodeV1 = { v: 1, answer: { sdp: ld.sdp, type: ld.type } }
      const answerCode = base64UrlEncodeString(JSON.stringify(code))
      emitState({ state: 'awaiting_offer_ack', answerCode })
      return answerCode
    } catch (e) {
      emitState({ state: 'error', error: e instanceof Error ? e.message : String(e) })
      throw e
    }
  }

  const acceptAnswer = async (answerCode: string): Promise<void> => {
    try {
      emitState({ state: 'connecting' })
      const pc0 = ensurePc()
      const decoded = safeJsonParse(base64UrlDecodeToString(answerCode)) as AnswerCodeV1
      if (!decoded || decoded.v !== 1) throw new Error('invalid answer code')
      const answer = decoded.answer
      if (!answer?.sdp || !answer?.type) throw new Error('invalid answer contents')
      await pc0.setRemoteDescription(answer)
      emitState({ state: 'connecting' })
    } catch (e) {
      emitState({ state: 'error', error: e instanceof Error ? e.message : String(e) })
      throw e
    }
  }

  const api = {
    close,
    send,
    getState: () => state,
    onState: (fn: (s: TransportStatus) => void) => {
      stateSubs.add(fn)
      fn(state)
      return () => stateSubs.delete(fn)
    },
    onMessage: (fn: (m: JsonValue) => void) => {
      msgSubs.add(fn)
      return () => msgSubs.delete(fn)
    },
    startOffer,
    acceptOfferAndMakeAnswer,
    acceptAnswer,
  }
  return api as unknown as WebRtcTransport
}

export type WebRtcTransportWithSignaling = WebRtcTransport & {
  startOffer: () => Promise<string>
  acceptOfferAndMakeAnswer: (offerCode: string) => Promise<string>
  acceptAnswer: (answerCode: string) => Promise<void>
}
