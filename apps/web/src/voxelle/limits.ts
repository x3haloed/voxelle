import type { EventV1 } from './rfc/types'

type LimitOk = { ok: true }
type LimitErr = { ok: false; error: string }
export type LimitResult = LimitOk | LimitErr

export const MAX_WIRE_MESSAGE_BYTES_DEFAULT = 256 * 1024
export const MAX_SIGNAL_WS_MESSAGE_BYTES = 64 * 1024
export const MAX_SIGNAL_SID_CHARS = 128
export const MAX_SIGNAL_SDP_CODE_CHARS = 128 * 1024

export const MAX_SYNC_HEADS = 256
export const MAX_SYNC_WANT_IDS = 256
export const MAX_SYNC_HAVE_EVENTS = 64

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null
}

function maxLen(s: string, n: number, name: string): LimitResult {
  if (s.length > n) return { ok: false, error: `${name} too long` }
  return { ok: true }
}

export function utf8ByteLen(s: string): number {
  return new TextEncoder().encode(s).length
}

export function isSafeWsSid(sid: string): boolean {
  const s = sid.trim()
  if (!s) return false
  if (s.length > MAX_SIGNAL_SID_CHARS) return false
  // Hex-only session ids (newSessionId()).
  return /^[0-9a-f]+$/i.test(s)
}

export class TokenBucket {
  private tokens: number
  private lastRefillMs: number
  private capacity: number
  private refillPerSec: number

  constructor(capacity: number, refillPerSec: number) {
    this.capacity = capacity
    this.refillPerSec = refillPerSec
    this.tokens = capacity
    this.lastRefillMs = Date.now()
  }

  allow(cost = 1): boolean {
    const now = Date.now()
    const elapsedSec = Math.max(0, now - this.lastRefillMs) / 1000
    if (elapsedSec > 0) {
      this.tokens = Math.min(this.capacity, this.tokens + elapsedSec * this.refillPerSec)
      this.lastRefillMs = now
    }

    if (this.tokens < cost) return false
    this.tokens -= cost
    return true
  }
}

export function checkEventLimits(ev: EventV1): LimitResult {
  // Keep these conservative for MVP; treat as local policy, not protocol validity.
  for (const [name, s, n] of [
    ['space_id', ev.space_id, 256],
    ['room_id', ev.room_id, 256],
    ['kind', ev.kind, 128],
    ['author_principal_id', ev.author_principal_id, 256],
    ['author_device_id', ev.author_device_id, 256],
    ['author_device_pub', ev.author_device_pub, 4096],
    ['sig', ev.sig, 2048],
    ['event_id', ev.event_id, 256],
  ] as const) {
    const r = maxLen(s, n, name)
    if (!r.ok) return r
  }

  if (Array.isArray(ev.prev)) {
    if (ev.prev.length > 64) return { ok: false, error: 'prev too long' }
    for (const p of ev.prev) {
      if (typeof p !== 'string') return { ok: false, error: 'prev entry not string' }
      const r = maxLen(p, 256, 'prev entry')
      if (!r.ok) return r
    }
  }

  const del = ev.delegation as any
  if (isObj(del)) {
    if (typeof del.sig === 'string') {
      const r = maxLen(del.sig, 2048, 'delegation.sig')
      if (!r.ok) return r
    }
    if (Array.isArray(del.scopes) && del.scopes.length > 64) return { ok: false, error: 'delegation.scopes too long' }
  }

  // Content-specific caps
  if (ev.kind === 'MSG_POST') {
    const body = ev.body
    if (!isObj(body) || typeof body.text !== 'string') return { ok: false, error: 'MSG_POST.body.text missing' }
    const text = body.text
    if (text.length > 2000) return { ok: false, error: 'message too long' }
  }

  return { ok: true }
}
