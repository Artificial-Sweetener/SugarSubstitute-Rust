// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Enforce native cache grants while upstream features retain keys and content verification. */
import fs from 'node:fs'
import path from 'node:path'
import { argument } from './arguments'

/** Stable host-localized failure; English detail remains in developer diagnostics. */
export class CacheStorageError extends Error {
  readonly statusCode = 'cacheFull'
  constructor() { super('Native cache byte grant exhausted') }
}
const MAX_ITEMS = 8192
const MAX_NAMES = 512 * 1024
const MAX_DEPTH = 512

/** One exclusive native grant. Charged high water includes queued writes and replacement scratch. */
export class CacheGrant {
  readonly root: string
  private readonly limit: number
  private readonly sizes = new Map<string, number>()
  private readonly names = new Map<string, string>()
  private readonly writers = new Set<string>()
  private nameBytes = 0
  private bytes = 0
  private reported = false
  private readonly pressure: () => void

  constructor(root: string, limit: number, pressure: () => void = () => {}) {
    this.pressure = pressure
    if (!path.isAbsolute(root) || !Number.isSafeInteger(limit) || limit <= 0) {
      throw new Error('Invalid native cache grant')
    }
    this.root = path.resolve(root)
    this.limit = limit
    this.directory(this.root)
    const visit = (directory: string, depth: number): void => {
      if (depth > MAX_DEPTH) throw new Error('Cache directory depth exceeded')
      for (const name of fs.readdirSync(directory)) {
        const file = path.join(directory, name)
        const metadata = fs.lstatSync(file, { bigint: true })
        this.register(file)
        if (metadata.isSymbolicLink()) throw new Error('Cache contains a link')
        if (metadata.isDirectory()) { this.directory(file); visit(file, depth + 1) }
        else { this.file(file); this.charge(file, Number(metadata.size)) }
      }
    }
    visit(this.root, 0)
  }

  contains(file: string): boolean {
    const relative = path.relative(this.root, path.resolve(file))
    return relative !== '' && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative)
  }

  private register(file: string): void {
    const relative = path.relative(this.root, path.resolve(file))
    if (Buffer.byteLength(file) > 4096 || !this.contains(file) || relative.split(path.sep).length > MAX_DEPTH || path.join(this.root, relative) !== file) {
      throw new Error('Cache path is outside its native grant')
    }
    for (const part of relative.split(path.sep)) {
      if (!part || /[<>:"|?*\u0000-\u001f]/.test(part) || /[. ]$/.test(part) ||
          /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)) {
        throw new Error('Cache path contains an unsupported name')
      }
    }
    const key = relative.toLowerCase()
    const original = this.names.get(key)
    if (original !== undefined) {
      if (original !== relative) throw new Error('Cache path aliases another entry')
      return
    }
    const bytes = Buffer.byteLength(relative) + 8
    if (this.names.size >= MAX_ITEMS || this.nameBytes + bytes > MAX_NAMES) {
      throw new Error('Cache inventory capacity exceeded')
    }
    this.names.set(key, relative)
    this.nameBytes += bytes
  }

  private directory(directory: string): void {
    const metadata = fs.lstatSync(directory, { bigint: true })
    if (!metadata.isDirectory() || metadata.isSymbolicLink() ||
        fs.realpathSync(directory).toLowerCase() !== directory.toLowerCase()) {
      throw new Error('Cache directories must not traverse links')
    }
  }

  private file(file: string): fs.BigIntStats | undefined {
    try {
      const metadata = fs.lstatSync(file, { bigint: true })
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.nlink !== 1n) {
        throw new Error('Cache files must have one regular directory link')
      }
      return metadata
    } catch (cause) {
      if ((cause as NodeJS.ErrnoException).code === 'ENOENT') return undefined
      throw cause
    }
  }

  private parent(file: string): void {
    this.register(file)
    const parts = path.relative(this.root, path.dirname(file)).split(path.sep).filter(Boolean)
    let directory = this.root
    this.directory(directory)
    for (const part of parts) {
      directory = path.join(directory, part)
      this.register(directory)
      try { fs.mkdirSync(directory) } catch (cause) {
        if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') throw cause
      }
      this.directory(directory)
    }
  }

  private charge(file: string, size: number): void {
    if (!Number.isSafeInteger(size) || size < 0) throw new Error('Invalid cache byte count')
    const before = this.sizes.get(file) ?? 0
    const next = this.bytes - before + size
    if (!Number.isSafeInteger(next) || next > this.limit) {
      if (!this.reported) { this.reported = true; this.pressure() }
      throw new CacheStorageError()
    }
    this.sizes.set(file, size)
    this.bytes = next
  }

  /** Reserve before touching a file, including bytes accepted into an asynchronous write queue. */
  reserve(file: string, size: number): void {
    this.parent(file)
    const current = Number(this.file(file)?.size ?? 0n)
    this.charge(file, Math.max(current, this.sizes.get(file) ?? 0, size))
  }

  /** Reconcile only after the actual stream has closed or the synchronous write completed. */
  settled(file: string): void {
    this.charge(file, Number(this.file(file)?.size ?? 0n))
  }

  /** Write one bounded reconstructible record with crash-safe replacement and no recursive cleanup. */
  write(file: string, data: string): void {
    const next = file + '.next'
    const size = Buffer.byteLength(data)
    this.reserve(file, Number(this.file(file)?.size ?? 0n))
    this.reserve(next, size)
    const descriptor = this.open(next)
    try { fs.ftruncateSync(descriptor, 0); fs.writeFileSync(descriptor, data); fs.fsyncSync(descriptor) }
    finally { fs.closeSync(descriptor) }
    fs.renameSync(next, file)
    this.settled(next)
    this.settled(file)
  }

  /** Open before truncation, then verify the actual handle instead of a preceding path lookup. */
  private open(file: string): number {
    this.parent(file)
    let descriptor: number
    try { descriptor = fs.openSync(file, 'wx+') } catch (cause) {
      if ((cause as NodeJS.ErrnoException).code !== 'EEXIST') throw cause
      descriptor = fs.openSync(file, 'r+')
    }
    try {
      const actual = fs.fstatSync(descriptor, { bigint: true })
      const expected = this.file(file)
      if (!expected || actual.nlink !== 1n || !actual.isFile() ||
          expected.dev !== actual.dev || expected.ino !== actual.ino) {
        throw new Error('Cache file identity changed before write')
      }
      return descriptor
    } catch (cause) { fs.closeSync(descriptor); throw cause }
  }

  /** One active stream per cache leaf; physical and queued high-water bytes share the same grant. */
  download(file: string, append: boolean, expected = 0): { fd: number; reserve: (size: number) => void; settled: () => void } {
    if (this.writers.has(file)) throw new Error('Cache file already has a writer')
    this.reserve(file, Math.max(expected, Number(this.file(file)?.size ?? 0n)))
    const fd = this.open(file)
    try { if (!append) fs.ftruncateSync(fd, 0) } catch (cause) { fs.closeSync(fd); throw cause }
    this.writers.add(file)
    return { fd, reserve: size => this.reserve(file, size), settled: () => {
      this.settled(file)
      this.writers.delete(file)
    } }
  }

  /** Cleanup belongs to the native owner after actual family retirement. Validate current pressure. */
  validate(): void {
    for (const file of this.sizes.keys()) {
      if (!this.writers.has(file)) this.settled(file)
      this.reserve(file, this.sizes.get(file) ?? 0)
    }
  }
}

let reportPressure: (namespace: 'downloads' | 'metadata') => void = () => {}
/** Private authenticated worker telemetry; cleanup still waits for actual native family retirement. */
export function observeCachePressure(report: typeof reportPressure): void { reportPressure = report }
let native: { downloads: CacheGrant; metadata: CacheGrant } | undefined
/** Require explicit native grants; no ambient workspace path is a cache fallback. */
export function cacheGrants(): { downloads: CacheGrant; metadata: CacheGrant } {
  if (!native) native = {
    downloads: new CacheGrant(argument('cache-downloads'), Number(argument('cache-download-bytes')), () => reportPressure('downloads')),
    metadata: new CacheGrant(argument('cache-metadata'), Number(argument('cache-metadata-bytes')), () => reportPressure('metadata')),
  }
  return native
}
/** Reconstructible metadata writers are the only upstream imports replaced by this boundary. */
export function writeFileSafe(file: string, data: string): void {
  const grant = cacheGrants().metadata
  if (!grant.contains(file)) throw new Error('Metadata write is outside its native cache grant')
  grant.write(file, data)
}
/** Feature-owned downloads to authored destinations retain upstream behavior outside cache storage. */
export function cacheDownload(file: string, append: boolean, expected = 0): ReturnType<CacheGrant['download']> | undefined {
  const grant = cacheGrants().downloads
  return grant.contains(file) ? grant.download(file, append, expected) : undefined
}
/** Resume sidecars use the same download grant and must fail observably before exceeding it. */
export function writeDownloadMeta(file: string, data: string): void {
  const grant = cacheGrants().downloads
  if (grant.contains(file)) grant.write(file, data)
  else fs.writeFileSync(file, data)
}
