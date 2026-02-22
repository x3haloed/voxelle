import canonicalize from 'canonicalize'

export function jcsBytes(value: unknown): Uint8Array {
  const s = canonicalize(value as any)
  if (typeof s !== 'string') throw new Error('canonicalize() returned non-string')
  return new TextEncoder().encode(s)
}

