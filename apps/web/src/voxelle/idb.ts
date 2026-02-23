import type { EventV1 } from './rfc/types'

type Db = IDBDatabase

const DB_NAME = 'voxelle'
const DB_VERSION = 1
const STORE_ROOMS = 'room_events'
const STORE_META = 'meta'
const META_ROOM_KEYS = 'room_keys_v1'

function reqToPromise<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error ?? new Error('indexeddb error'))
  })
}

function txDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve()
    tx.onabort = () => reject(tx.error ?? new Error('indexeddb tx aborted'))
    tx.onerror = () => reject(tx.error ?? new Error('indexeddb tx error'))
  })
}

let dbPromise: Promise<Db> | null = null

async function openDb(): Promise<Db> {
  if (dbPromise) return dbPromise
  dbPromise = new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains(STORE_ROOMS)) db.createObjectStore(STORE_ROOMS)
      if (!db.objectStoreNames.contains(STORE_META)) db.createObjectStore(STORE_META)
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error ?? new Error('indexeddb open failed'))
  })
  return dbPromise
}

export function roomKey(spaceId: string, roomId: string): string {
  return `${encodeURIComponent(spaceId)}|${encodeURIComponent(roomId)}`
}

export function parseRoomKey(k: string): { spaceId: string; roomId: string } | null {
  const [a, b] = k.split('|')
  if (!a || !b) return null
  try {
    return { spaceId: decodeURIComponent(a), roomId: decodeURIComponent(b) }
  } catch {
    return null
  }
}

export async function listRoomKeys(): Promise<string[]> {
  const db = await openDb()
  const tx = db.transaction([STORE_META], 'readonly')
  const meta = tx.objectStore(STORE_META)
  const keys = (await reqToPromise(meta.get(META_ROOM_KEYS))) as unknown
  await txDone(tx)
  return Array.isArray(keys) ? (keys.filter((x) => typeof x === 'string') as string[]) : []
}

export async function loadRoomEvents(k: string): Promise<EventV1[] | null> {
  const db = await openDb()
  const tx = db.transaction([STORE_ROOMS], 'readonly')
  const st = tx.objectStore(STORE_ROOMS)
  const v = (await reqToPromise(st.get(k))) as unknown
  await txDone(tx)
  return Array.isArray(v) ? (v as EventV1[]) : null
}

export async function saveRoomEvents(k: string, events: EventV1[]): Promise<void> {
  const db = await openDb()
  const tx = db.transaction([STORE_ROOMS, STORE_META], 'readwrite')
  const rooms = tx.objectStore(STORE_ROOMS)
  const meta = tx.objectStore(STORE_META)

  rooms.put(events, k)

  const existing = (await reqToPromise(meta.get(META_ROOM_KEYS))) as unknown
  const keys = Array.isArray(existing) ? (existing.filter((x) => typeof x === 'string') as string[]) : []
  if (!keys.includes(k)) {
    meta.put([...keys, k], META_ROOM_KEYS)
  }

  await txDone(tx)
}

