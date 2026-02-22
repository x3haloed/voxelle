import { concatBytes, utf8 } from './bytes'

export function netstring(bytes: Uint8Array): Uint8Array {
  const len = utf8(String(bytes.byteLength))
  return concatBytes([len, utf8(':'), bytes, utf8(',')])
}

export class NetstringWriter {
  private chunks: Uint8Array[] = []

  writePrefix(prefix: string) {
    this.chunks.push(utf8(prefix))
  }

  writeBytes(bytes: Uint8Array) {
    this.chunks.push(netstring(bytes))
  }

  writeStr(s: string) {
    this.writeBytes(utf8(s))
  }

  writeInt(n: number) {
    if (!Number.isFinite(n) || !Number.isInteger(n)) throw new Error('int must be finite integer')
    this.writeStr(String(n))
  }

  writeCount(n: number) {
    if (!Number.isFinite(n) || !Number.isInteger(n) || n < 0) throw new Error('count must be >= 0 int')
    this.writeStr(String(n))
  }

  finish(): Uint8Array {
    return concatBytes(this.chunks)
  }
}

