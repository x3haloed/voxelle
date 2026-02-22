import type { EventV1 } from './rfc/types'
import { topoSortDeterministic } from './dag'
import type { InviteV1 } from './rfc/invite'
import { validateInvite } from './rfc/invite'

export type GovernanceStateV1 = {
  members: Set<string>
  banned: Set<string>
}

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null
}

async function acceptMemberJoin(ev: EventV1): Promise<boolean> {
  const body = ev.body as any
  if (!isObj(body)) return false
  if (body.principal_id !== ev.author_principal_id) return false
  if (typeof body.principal_pub !== 'string' || !body.principal_pub) return false
  if (body.principal_pub !== ev.delegation.principal_pub) return false
  const invite = body.invite as InviteV1
  const ok = await validateInvite(invite)
  if (!ok.ok) return false
  if (invite.space_id !== ev.space_id) return false
  return true
}

export async function deriveGovernanceState(governanceEvents: EventV1[]): Promise<GovernanceStateV1> {
  const byId = new Map(governanceEvents.map((e) => [e.event_id, e]))
  const orderedIds = topoSortDeterministic(governanceEvents)

  const members = new Set<string>()
  const banned = new Set<string>()

  for (const id of orderedIds) {
    const ev = byId.get(id)
    if (!ev) continue

    if (ev.kind === 'MEMBER_JOIN') {
      if (await acceptMemberJoin(ev)) {
        members.add(ev.author_principal_id)
      }
      continue
    }

    if (ev.kind === 'MEMBER_BAN') {
      const body = ev.body as any
      const pid = typeof body?.principal_id === 'string' ? body.principal_id : ''
      if (pid) banned.add(pid)
      continue
    }

    if (ev.kind === 'MEMBER_UNBAN') {
      const body = ev.body as any
      const pid = typeof body?.principal_id === 'string' ? body.principal_id : ''
      if (pid) banned.delete(pid)
      continue
    }
  }

  return { members, banned }
}

