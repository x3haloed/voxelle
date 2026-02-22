import type { EventV1 } from './rfc/types'

export type RoomDag = {
  byId: Map<string, EventV1>
  children: Map<string, Set<string>>
  indegree: Map<string, number>
}

export function buildDag(events: EventV1[]): RoomDag {
  const byId = new Map<string, EventV1>()
  for (const ev of events) {
    if (typeof ev?.event_id === 'string' && ev.event_id) byId.set(ev.event_id, ev)
  }

  const children = new Map<string, Set<string>>()
  const indegree = new Map<string, number>()
  for (const id of byId.keys()) {
    children.set(id, new Set())
    indegree.set(id, 0)
  }

  for (const ev of byId.values()) {
    const childId = ev.event_id
    for (const parentId of ev.prev || []) {
      if (!byId.has(parentId)) continue
      children.get(parentId)!.add(childId)
      indegree.set(childId, (indegree.get(childId) ?? 0) + 1)
    }
  }

  return { byId, children, indegree }
}

export function topoSortDeterministic(events: EventV1[]): string[] {
  const dag = buildDag(events)
  const byId = dag.byId
  const indegree = new Map(dag.indegree)

  const ready: string[] = []
  for (const [id, deg] of indegree.entries()) {
    if (deg === 0) ready.push(id)
  }

  const pickOrder = (a: string, b: string): number => {
    const ea = byId.get(a)
    const eb = byId.get(b)
    const ta = typeof ea?.ts === 'number' ? ea.ts : 0
    const tb = typeof eb?.ts === 'number' ? eb.ts : 0
    if (ta !== tb) return ta - tb
    return a.localeCompare(b)
  }

  const out: string[] = []
  while (ready.length > 0) {
    ready.sort(pickOrder)
    const id = ready.shift()!
    out.push(id)

    const kids = dag.children.get(id)
    if (!kids) continue
    for (const k of kids) {
      const d = (indegree.get(k) ?? 0) - 1
      indegree.set(k, d)
      if (d === 0) ready.push(k)
    }
  }

  // If there are cycles (shouldn't happen), fall back to stable sort for the remaining nodes.
  if (out.length !== byId.size) {
    const remaining = [...byId.keys()].filter((id) => !out.includes(id))
    remaining.sort(pickOrder)
    out.push(...remaining)
  }

  return out
}

export function computeHeads(events: EventV1[]): string[] {
  const dag = buildDag(events)
  const heads: string[] = []
  for (const id of dag.byId.keys()) {
    const kids = dag.children.get(id)
    if (!kids || kids.size === 0) heads.push(id)
  }
  heads.sort()
  return heads
}

