import { useEffect, useState } from 'react'
import { isTauri, tauriInvoke } from '../voxelle/tauri'

type Status = { active_version: string; feed_url: string; port: number }
type Check = { available: boolean; version?: string | null; zip_url?: string | null; sha256?: string | null }
type Download = { activated_version: string }

export function WebUpdateBadge() {
  const [supported, setSupported] = useState(false)
  const [status, setStatus] = useState<Status | null>(null)
  const [check, setCheck] = useState<Check | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [err, setErr] = useState<string | null>(null)

  useEffect(() => {
    if (!isTauri()) return
    ;(async () => {
      try {
        const st = await tauriInvoke<Status>('web_update_status')
        setSupported(true)
        setStatus(st)
      } catch {
        // Running in browser or dev Tauri without updater wired; hide.
        setSupported(false)
      }
    })()
  }, [])

  if (!supported) return null

  const v = status?.active_version || 'unknown'
  const feed = status?.feed_url || ''

  return (
    <div className="row" style={{ gap: 8, alignItems: 'center' }}>
      <span className="pill">{`web ${v}`}</span>
      {check?.available ? <span className="pill accent">{`update → ${check.version}`}</span> : null}
      <button
        className="pill"
        onClick={async () => {
          const next = window.prompt('Web update feed URL (manifest JSON). Leave empty to disable.', feed) ?? feed
          setErr(null)
          try {
            setBusy('setting')
            await tauriInvoke('web_update_set_feed', { url: next })
            const st = await tauriInvoke<Status>('web_update_status')
            setStatus(st)
          } catch (e) {
            setErr(e instanceof Error ? e.message : String(e))
          } finally {
            setBusy(null)
          }
        }}
        disabled={!!busy}
      >
        {busy === 'setting' ? 'Setting…' : 'Feed'}
      </button>
      <button
        className="pill"
        onClick={async () => {
          setErr(null)
          try {
            setBusy('checking')
            const r = await tauriInvoke<Check>('web_update_check')
            setCheck(r)
          } catch (e) {
            setErr(e instanceof Error ? e.message : String(e))
          } finally {
            setBusy(null)
          }
        }}
        disabled={!!busy}
      >
        {busy === 'checking' ? 'Checking…' : 'Check'}
      </button>
      {check?.available ? (
        <button
          className="pill accent"
          onClick={async () => {
            setErr(null)
            try {
              setBusy('downloading')
              const r = await tauriInvoke<Download>('web_update_download')
              const st = await tauriInvoke<Status>('web_update_status')
              setStatus(st)
              setCheck({ ...check, available: false })
              window.alert(`Update downloaded: ${r.activated_version}. Refresh to apply.`)
            } catch (e) {
              setErr(e instanceof Error ? e.message : String(e))
            } finally {
              setBusy(null)
            }
          }}
          disabled={!!busy}
        >
          {busy === 'downloading' ? 'Downloading…' : 'Download'}
        </button>
      ) : null}
      <button className="pill" onClick={() => window.location.reload()} disabled={!!busy}>
        Refresh
      </button>
      {err ? (
        <span className="muted" style={{ fontSize: 12, color: 'var(--danger)', maxWidth: 340, overflow: 'hidden' }}>
          {err}
        </span>
      ) : null}
    </div>
  )
}
