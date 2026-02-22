import type { InviteV1 } from './invite'

export type RendezvousHint =
  | { kind: 'signal-ws'; url: string; sid: string }
  | { kind: 'unknown' }

export function parseInviteRendezvous(invite: InviteV1): RendezvousHint | null {
  const rendezvous = (invite.bootstrap as any)?.rendezvous
  if (!Array.isArray(rendezvous)) return null

  for (const s of rendezvous) {
    if (typeof s !== 'string') continue
    if (!s.startsWith('signal-ws:')) continue
    // Format: signal-ws:<wsUrl>#sid=<sid>
    const rest = s.slice('signal-ws:'.length)
    const [urlPart, hashPart = ''] = rest.split('#', 2)
    const url = urlPart.trim()
    if (!url) continue
    const params = new URLSearchParams(hashPart)
    const sid = params.get('sid')?.trim() ?? ''
    if (!sid) continue
    return { kind: 'signal-ws', url, sid }
  }

  return null
}

