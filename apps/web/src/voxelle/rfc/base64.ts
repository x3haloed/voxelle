const B64ABC = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

export function base64Encode(bytes: Uint8Array): string {
  let out = ''
  let i = 0
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2]
    out += B64ABC[(n >>> 18) & 63]
    out += B64ABC[(n >>> 12) & 63]
    out += B64ABC[(n >>> 6) & 63]
    out += B64ABC[n & 63]
  }
  const rem = bytes.length - i
  if (rem === 1) {
    const n = bytes[i] << 16
    out += B64ABC[(n >>> 18) & 63]
    out += B64ABC[(n >>> 12) & 63]
    out += '=='
  } else if (rem === 2) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8)
    out += B64ABC[(n >>> 18) & 63]
    out += B64ABC[(n >>> 12) & 63]
    out += B64ABC[(n >>> 6) & 63]
    out += '='
  }
  return out
}

export function base64UrlNoPad(bytes: Uint8Array): string {
  return base64Encode(bytes).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')
}

