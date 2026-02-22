import type { EventV1 } from './rfc/types'

type LimitOk = { ok: true }
type LimitErr = { ok: false; error: string }
export type LimitResult = LimitOk | LimitErr

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null
}

function maxLen(s: string, n: number, name: string): LimitResult {
  if (s.length > n) return { ok: false, error: `${name} too long` }
  return { ok: true }
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
