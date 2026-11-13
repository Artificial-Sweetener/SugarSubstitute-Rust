// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Native download storage; upstream retains networking, resume metadata and content verification. */
import path from 'node:path'
import { argument } from './arguments'
import { NativeExchange, type Data } from './native-exchange'

const CHUNK = 24 * 1024
const QUEUE_BYTES = 1024 * 1024
const MAXIMUM = 24 * 1024 * 1024 * 1024

/** Native facts remain available while the host supplies localized product status text. */
export class DownloadStorageError extends Error {
  readonly statusCode: 'cacheFull' | 'setupFailed'
  constructor(message: string, readonly code?: string, options?: ErrorOptions) {
    super(message, options)
    this.statusCode = code === 'ENOSPC' ? 'cacheFull' : 'setupFailed'
  }
}
/** Await settlement before extraction or feature completion; dropping this object proves no release. */
export type DownloadFile = {
  readonly length: number
  write(bytes: Uint8Array): Promise<void>
  settled(): Promise<void>
}

/** One original host command owns a bounded queue through the authenticated native storage lane. */
export class NativeDownloads {
  private readonly exchange: NativeExchange
  private queue: Promise<void> = Promise.resolve()
  private queued = 0
  private queuedBytes = 0
  private generation = 0
  readonly root: string
  constructor(send: (id: string, data: Data) => void, root: string) {
    if (!path.isAbsolute(root)) throw new DownloadStorageError('Native download root is absent')
    this.root = path.resolve(root)
    this.exchange = new NativeExchange(send, message => new DownloadStorageError(message))
  }
  begin(id: string): void {
    if (this.queued) throw new DownloadStorageError('Download operations remain queued')
    this.exchange.begin(id)
    this.generation++
  }
  close(cause?: Error): void { this.generation++; this.exchange.close(cause) }
  accept(command: Parameters<NativeExchange['accept']>[0]): void { this.exchange.accept(command) }
  /** This is inert path classification; only the prepared Rust capability authorizes native I/O. */
  contains(file: string): boolean {
    if (!path.isAbsolute(file)) return false
    const name = path.relative(this.root, path.resolve(file))
    return name !== '' && name !== '..' && !name.startsWith('..' + path.sep) && !path.isAbsolute(name)
  }
  async entry(key: string): Promise<string> {
    const result = await this.enqueue(0, () => this.call({ op: 'entry', key }))
    if (typeof result.path !== 'string' || !this.contains(result.path) || !/^[a-f0-9]{64}$/.test(path.relative(this.root, result.path))) {
      throw new DownloadStorageError('Native download entry changed identity')
    }
    return result.path
  }
  async length(file: string): Promise<number | undefined> {
    const name = this.name(file)
    const result = await this.enqueue(0, () => this.call({ op: 'length', name }))
    if (result.length === null) return undefined
    if (!Number.isSafeInteger(result.length) || (result.length as number) < 0 || (result.length as number) > MAXIMUM) {
      throw new DownloadStorageError('Invalid native payload length')
    }
    return result.length as number
  }
  async readFile(file: string): Promise<string> {
    const name = this.name(file)
    const result = await this.enqueue(0, () => this.call({ op: 'read', name }))
    if (typeof result.hex !== 'string' || result.hex.length > 16384 || !/^(?:[0-9a-f]{2})*$/.test(result.hex)) {
      throw new DownloadStorageError('Invalid native download record')
    }
    return Buffer.from(result.hex, 'hex').toString('utf8')
  }
  async writeFile(file: string, data: string): Promise<void> {
    const name = this.name(file), length = Buffer.byteLength(data)
    if (length > 8192) throw new DownloadStorageError('Download record exceeds its bound')
    await this.enqueue(length, () => this.call({ op: 'write', name, hex: Buffer.from(data).toString('hex') }))
  }
  async remove(file: string): Promise<void> {
    const name = this.name(file)
    await this.enqueue(0, () => this.call({ op: 'remove', name }))
  }
  /** Open only after response framing selects the existing physical prefix and expected extent. */
  async open(file: string, append: boolean, expected = 0): Promise<DownloadFile> {
    const name = this.name(file)
    if (!Number.isSafeInteger(expected) || expected < 0 || expected > MAXIMUM) throw new DownloadStorageError('Invalid download extent')
    const result = await this.enqueue(0, () => this.call({ op: 'open', name, expected }))
    try {
      if (!Number.isSafeInteger(result.length) || (result.length as number) < 0 || (result.length as number) > MAXIMUM) {
        throw new DownloadStorageError('Invalid native payload prefix')
      }
      if (!append) await this.enqueue(0, () => this.call({ op: 'reset', name }))
    } catch (cause) {
      try { await this.enqueue(0, () => this.call({ op: 'close', name })) }
      catch (cleanup) { throw new AggregateError([cause, cleanup], 'Native payload preparation and retirement failed') }
      throw cause
    }
    let position = append ? result.length as number : 0
    let closed = false
    let settling: Promise<void> | undefined
    return {
      length: position,
      write: async bytes => {
        if (closed || bytes.byteLength > QUEUE_BYTES) throw new DownloadStorageError('Native payload is closed or input exceeds its queue bound')
        await this.enqueue(bytes.byteLength, async () => {
          for (let start = 0; start < bytes.byteLength; start += CHUNK) {
            const chunk = Buffer.from(bytes.buffer, bytes.byteOffset + start, Math.min(CHUNK, bytes.byteLength - start))
            await this.call({ op: 'chunk', name, offset: position, hex: chunk.toString('hex') })
            position += chunk.byteLength
          }
        })
      },
      settled: () => {
        if (settling) return settling
        closed = true
        settling = this.enqueue(0, () => this.call({ op: 'close', name })).then(() => {})
        return settling
      },
    }
  }
  async validate(): Promise<void> { await this.enqueue(0, () => this.call({ op: 'validate' })) }
  /** Observe all queued native effects before the worker publishes its terminal command event. */
  async drain(): Promise<void> { await this.queue }
  private name(file: string): string {
    if (!this.contains(file)) throw new DownloadStorageError('Foreign native download selector')
    return path.relative(this.root, path.resolve(file)).split(path.sep).join('/')
  }
  private enqueue<T>(bytes: number, body: () => Promise<T>): Promise<T> {
    if (this.queued >= 64 || this.queuedBytes + bytes > QUEUE_BYTES) return Promise.reject(new DownloadStorageError('Native download queue is full', 'EBUSY'))
    const generation = this.generation
    this.queued++; this.queuedBytes += bytes
    const task = this.queue.then(async () => {
      if (generation !== this.generation) throw new DownloadStorageError('Native download command retired')
      return body()
    }).finally(() => { this.queued--; this.queuedBytes -= bytes })
    this.queue = task.then(() => {}, () => {})
    return task
  }
  private async call(operation: Data): Promise<Data> {
    try {
      const frames = await this.exchange.call(operation)
      if (frames.length !== 1) throw new DownloadStorageError('Download reply was fragmented')
      return frames[0]
    } catch (cause) {
      if (cause instanceof Error && (cause as NodeJS.ErrnoException).code === 'ENOSPC') {
        throw Object.assign(new DownloadStorageError(cause.message, 'ENOSPC', { cause }), { osCode: (cause as Error & { osCode?: number }).osCode })
      }
      throw cause
    }
  }
}
let native: NativeDownloads | undefined
/** Bind only to the authenticated host; an ambient path never creates a native storage capability. */
export function bindNativeDownloads(client: NativeDownloads): void { native = client }
export function nativeDownloads(): NativeDownloads {
  if (!native) throw new DownloadStorageError('Native download transport is absent')
  return native
}
export function downloadsRoot(): string { return argument('cache-downloads') }
