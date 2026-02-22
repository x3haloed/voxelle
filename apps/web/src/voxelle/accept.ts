import type { EventV1 } from './rfc/types'
import { validateEvent } from './rfc/validate'
import { deriveGovernanceState } from './governance'

export type AcceptOk<T> = { ok: true; value: T }
export type AcceptErr = { ok: false; error: string }
export type AcceptResult<T> = AcceptOk<T> | AcceptErr

export type RoomEventsProvider = (spaceId: string, roomId: string) => EventV1[]

export async function acceptEvent(ev: EventV1, getRoomEvents: RoomEventsProvider): Promise<AcceptResult<EventV1>> {
  const cryptoOk = await validateEvent(ev)
  if (!cryptoOk.ok) return cryptoOk

  // Governance room: allow MEMBER_JOIN (invite-validated in governance state machine),
  // and allow other governance actions only by Space Root for now (MVP).
  if (ev.room_id === 'governance') {
    if (ev.kind === 'MEMBER_JOIN') {
      // We accept MEMBER_JOIN if it would be accepted by governance rules.
      const gs = await deriveGovernanceState([ev])
      if (!gs.members.has(ev.author_principal_id)) return { ok: false, error: 'governance: invalid MEMBER_JOIN' }
      return { ok: true, value: ev }
    }
    if (ev.author_principal_id !== ev.space_id) {
      return { ok: false, error: 'governance: only space root may emit non-join governance events (MVP)' }
    }
    return { ok: true, value: ev }
  }

  // Non-governance rooms: must be admitted member and not banned.
  const gov = getRoomEvents(ev.space_id, 'governance')
  const acceptedGov: EventV1[] = []
  for (const g of gov) {
    const ok = await validateEvent(g)
    if (!ok.ok) continue
    // Additional join-specific checks happen in deriveGovernanceState.
    acceptedGov.push(ok.value)
  }
  const gs = await deriveGovernanceState(acceptedGov)
  if (!gs.members.has(ev.author_principal_id)) return { ok: false, error: 'not a member' }
  if (gs.banned.has(ev.author_principal_id)) return { ok: false, error: 'banned' }

  return { ok: true, value: ev }
}

