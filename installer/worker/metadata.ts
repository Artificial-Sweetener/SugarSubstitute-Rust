// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Native metadata streams and synchronous snapshots for the upstream schema owners. */
import { diagnostic } from './diagnostics.mjs'
import path from 'node:path'
import { argument } from './arguments'
import { NativeExchange, type Data } from './native-exchange'
const NAMES = ['fetch-cache.json', 'torch-stack-cache.json', 'torch-index-manifest-cache.json']
const LIMIT = 256 * 1024 * 1024
/** Storage failures retain native facts while the host provides localized product text. */
export class MetadataError extends Error {
  readonly statusCode: 'cacheFull' | 'setupFailed'
  constructor(message: string, readonly code?: string, options?: ErrorOptions) {
    super(message, options); this.statusCode = code === 'ENOSPC' ? 'cacheFull' : 'setupFailed'
  }
}
/** No local filesystem fallback. One original command owns a bounded queue of complete transfers. */
export class NativeMetadata {
  private readonly exchange: NativeExchange
  private readonly snapshots = new Map<string, string>()
  private queue: Promise<void> = Promise.resolve()
  private queued = 0
  private queuedBytes = 0
  private generation = 0
  private loaded = false
  constructor(send: (id: string, data: Data) => void, private readonly root: string) {
    if (!path.isAbsolute(root)) throw new MetadataError('Native metadata root is absent')
    this.exchange = new NativeExchange(send, message => new MetadataError(message))
  }
  begin(id: string): void {
    if (this.queued) throw new MetadataError('Metadata command still has queued transfers')
    this.exchange.begin(id); this.generation++
  }
  close(cause?: Error): void { this.generation++; this.exchange.close(cause) }
  accept(command: Parameters<NativeExchange['accept']>[0]): void { this.exchange.accept(command) }
  /** Hydrate the original process snapshot before upstream synchronous reads can occur. */
  async load(): Promise<void> {
    if (this.loaded) return
    for (const name of NAMES) {
      try { this.snapshots.set(name, await this.read(name)) }
      catch (cause) { diagnostic('warn', 'Native metadata cache miss', name, cause) }
    }
    this.loaded = true
  }
  /** Feature owners parse their original bytes; a miss follows their existing fallback policy. */
  readFile(file: string): string {
    const name = this.name(file)
    if (!this.loaded) throw new MetadataError('Native metadata snapshot is not loaded')
    const value = this.snapshots.get(name)
    if (value === undefined) throw new MetadataError('Native metadata cache miss', 'ENOENT')
    return value
  }
  /** Publication completes before a feature updates its snapshot or sends its terminal event. */
  async writeFile(file: string, data: string): Promise<void> {
    const name = this.name(file)
    const length = Buffer.byteLength(data)
    if (length > LIMIT) throw new MetadataError('Metadata exceeds its byte grant', 'ENOSPC')
    if (this.queuedBytes + length > LIMIT) throw new MetadataError('Metadata queued byte capacity exhausted')
    this.queuedBytes += length
    try { await this.enqueue(async () => {
      const bytes = Buffer.from(data)
      await this.call({ op: 'begin', name, length: bytes.length })
      for (let offset = 0; offset < bytes.length; offset += 8192) {
        await this.call({ op: 'chunk', offset, hex: bytes.subarray(offset, offset + 8192).toString('hex') })
      }
      await this.call({ op: 'commit' })
      this.snapshots.set(name, data)
    }) } finally { this.queuedBytes -= length }
  }
  /** Observe every queued operation before retiring this command's authenticated exchange. */
  async drain(): Promise<void> { await this.queue }
  private name(file: string): string {
    const name = path.relative(this.root, file)
    if (!path.isAbsolute(file) || !NAMES.includes(name)) throw new MetadataError('Foreign metadata record')
    return name
  }
  private async read(name: string): Promise<string> {
    return this.enqueue(async () => {
      const chunks: Buffer[] = []
      let result = await this.call({ op: 'read', name }), size = 0
      for (;;) {
        if (typeof result.hex !== 'string' || !/^(?:[0-9a-f]{2})*$/.test(result.hex) || result.hex.length > 16384 || typeof result.done !== 'boolean') {
          throw new MetadataError('Invalid native metadata chunk')
        }
        size += result.hex.length / 2
        if (size > LIMIT || !result.done && !result.hex.length) throw new MetadataError('Metadata read exceeded its bound')
        chunks.push(Buffer.from(result.hex, 'hex'))
        if (result.done) return Buffer.concat(chunks, size).toString('utf8')
        result = await this.call({ op: 'next' })
      }
    })
  }
  private enqueue<T>(body: () => Promise<T>): Promise<T> {
    if (this.queued >= 8) return Promise.reject(new MetadataError('Metadata queue is full'))
    const generation = this.generation
    this.queued++
    const task = this.queue.then(async () => {
      if (generation !== this.generation) throw new MetadataError('Metadata command retired')
      try { return await body() }
      catch (cause) {
        if (generation === this.generation) {
          try { await this.call({ op: 'abort' }) }
          catch (abortCause) { throw new AggregateError([cause, abortCause], 'Metadata transfer failed and could not retire') }
        }
        throw cause
      }
    }).finally(() => { this.queued-- })
    // Callers observe the task's failure; the queue continues so one cache miss cannot hide later records.
    this.queue = task.then(() => {}, () => {})
    return task
  }
  private async call(operation: Data): Promise<Data> {
    try {
      const frames = await this.exchange.call(operation)
      if (frames.length !== 1) throw new MetadataError('Metadata reply was fragmented')
      return frames[0]
    } catch (cause) {
      if (cause instanceof Error && (cause as NodeJS.ErrnoException).code === 'ENOSPC') {
        throw Object.assign(new MetadataError(cause.message, 'ENOSPC', { cause }), { osCode: (cause as Error & { osCode?: number }).osCode })
      }
      throw cause
    }
  }
}
let native: NativeMetadata | undefined
/** Bind only to the authenticated host transport; paths never instantiate storage authority. */
export function bindNativeMetadata(client: NativeMetadata): void { native = client }
function client(): NativeMetadata {
  if (!native) throw new MetadataError('Native metadata transport is absent')
  return native
}
export function readFileSafe(file: string): string { return client().readFile(file) }
export async function writeFileSafe(file: string, data: string): Promise<void> { await client().writeFile(file, data) }
export function metadataRoot(): string { return argument('cache-metadata') }
