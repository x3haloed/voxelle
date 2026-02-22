import type { InviteV1 } from './rfc/invite'

export function encodeInviteToFragment(invite: InviteV1): string {
  const json = JSON.stringify(invite)
  const bytes = new TextEncoder().encode(json)
  let bin = ''
  const chunk = 0x8000
  for (let i = 0; i < bytes.length; i += chunk) {
    bin += String.fromCharCode(...bytes.subarray(i, i + chunk))
  }
  const b64 = btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
  return `#invite=${b64}`
}

export function decodeInviteFromUrl(url: string): InviteV1 | null {
  try {
    const u = new URL(url, window.location.origin)
    const hash = u.hash.startsWith('#') ? u.hash.slice(1) : u.hash
    const params = new URLSearchParams(hash)
    const b64u = params.get('invite')
    if (!b64u) return null
    let b64 = b64u.replace(/-/g, '+').replace(/_/g, '/')
    while (b64.length % 4 !== 0) b64 += '='
    const bin = atob(b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    const json = new TextDecoder().decode(bytes)
    return JSON.parse(json) as InviteV1
  } catch {
    return null
  }
}

