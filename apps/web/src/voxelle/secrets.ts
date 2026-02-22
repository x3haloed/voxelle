import { isTauri, tauriInvoke } from './tauri'

export function secretsAvailable(): boolean {
  return isTauri()
}

export async function secretGet(key: string): Promise<string | null> {
  if (!secretsAvailable()) return null
  return (await tauriInvoke<string | null>('voxelle_secret_get', { key })) ?? null
}

export async function secretSet(key: string, value: string): Promise<void> {
  if (!secretsAvailable()) throw new Error('secrets not available')
  await tauriInvoke('voxelle_secret_set', { key, value })
}

export async function secretDelete(key: string): Promise<void> {
  if (!secretsAvailable()) return
  await tauriInvoke('voxelle_secret_delete', { key })
}

