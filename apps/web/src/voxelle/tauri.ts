type TauriGlobal = {
  core?: {
    invoke?: <T>(cmd: string, args?: any) => Promise<T>
  }
}

function tauriGlobal(): TauriGlobal | null {
  const g = (window as any).__TAURI__ as TauriGlobal | undefined
  if (!g?.core?.invoke) return null
  return g
}

export function isTauri(): boolean {
  return !!tauriGlobal()
}

export async function tauriInvoke<T>(cmd: string, args?: any): Promise<T> {
  const g = tauriGlobal()
  if (!g?.core?.invoke) throw new Error('tauri invoke not available')
  return g.core.invoke<T>(cmd, args)
}

