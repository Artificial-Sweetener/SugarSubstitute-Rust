// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Exchange bounded ordinary record operations with the active host-selected native owner. */
import path from 'node:path'
const MAX_RECORD = 65536
import { NativeExchange, type Data } from './native-exchange'
/** Native record failures retain developer detail while the host supplies localized product copy. */
export class NativeRecordError extends Error { readonly statusCode = 'setupFailed' }
/** Selected ordinary-record operations share one ordered transport exchange. */
export class NativeRecords {
  private selected: string | undefined
  private root: string | undefined
  private readonly exchange: NativeExchange
  constructor(send: (id: string, data: Data) => void) {
    this.exchange = new NativeExchange(send, message => new NativeRecordError(message))
  }
  begin(id: string, selected?: string): void {
    this.exchange.begin(id); this.selected = selected; this.root = undefined
  }
  close(cause?: Error): void {
    this.exchange.close(cause); this.selected = undefined; this.root = undefined
  }
  accept(command: Parameters<NativeExchange['accept']>[0]): void { this.exchange.accept(command) }
  private call(operation: Data): Promise<Data[]> { return this.exchange.call(operation) }
  /** Pin exactly the selected local folder. Its parent must already exist. */
  async open(directory: string, create: boolean): Promise<string | undefined> {
    if (!path.isAbsolute(directory) || path.parse(directory).root === directory ||
        directory.split(/[\\/]/).some(part => part === '..' || part === '.') ||
        !this.selected || !samePath(path.resolve(directory), path.resolve(this.selected))) {
      throw new NativeRecordError('Managed records require the host-selected absolute folder')
    }
    const [result] = await this.call({ op: 'open', create })
    if (result.absent === true && !create) return undefined
    if (typeof result.path !== 'string' || !samePath(path.resolve(result.path), path.resolve(directory))) {
      throw new NativeRecordError('Native record folder changed')
    }
    this.root = path.resolve(result.path)
    return this.root
  }
  /** Enumerate bounded pages; unknown and interrupted children remain with their feature owner. */
  async *entries(): AsyncGenerator<{ name: string; directory: boolean }> {
    for (let restart = true;; restart = false) {
      const [result] = await this.call({ op: 'page', restart })
      if (!Array.isArray(result.entries) || result.entries.length > 32) throw new NativeRecordError('Invalid native record inventory')
      if (!result.entries.length) return
      for (const entry of result.entries) {
        if (typeof entry.name !== 'string' || Buffer.byteLength(entry.name) > 1024 || typeof entry.directory !== 'boolean') {
          throw new NativeRecordError('Invalid native record entry')
        }
        yield entry
      }
    }
  }
  /** Keep the native read revision paired with the exact original bytes for later replacement. */
  async read(child?: string): Promise<{ text: string; revision: number }> {
    const frames = await this.call({ op: 'read', child: child ?? null, leaf: leaf(child) })
    let hex = '', revision: number | undefined
    for (const frame of frames) {
      if (typeof frame.hex !== 'string' || !/^(?:[0-9a-f]{2})*$/.test(frame.hex) || frame.hex.length > 16384 ||
          !Number.isSafeInteger(frame.revision) || Number(frame.revision) <= 0 ||
          revision !== undefined && revision !== frame.revision) throw new NativeRecordError('Invalid native record bytes')
      revision = Number(frame.revision); hex += frame.hex
      if (hex.length > MAX_RECORD * 2) throw new NativeRecordError('Native record exceeds its bound')
    }
    if (revision === undefined) throw new NativeRecordError('Native record revision absent')
    return { text: Buffer.from(hex, 'hex').toString('utf8'), revision }
  }
  /** Create an independent absent attempt; the native owner verifies its actual directory. */
  async createChild(name: string): Promise<string> {
    const [result] = await this.call({ op: 'create_child', name })
    const expected = this.child(name)
    if (typeof result.path !== 'string' || !samePath(path.resolve(result.path), expected)) throw new NativeRecordError('Native attempt folder changed')
    return expected
  }
  /** Return an observed path for feature comparisons; it is not a storage capability. */
  child(name: string): string {
    if (!this.root || !/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(name)) throw new NativeRecordError('Invalid native attempt identity')
    return path.join(this.root, name)
  }
  /** Chunk before transfer, retaining the complete original bound and native commit acknowledgment. */
  async publish(text: string, child?: string, revision?: number): Promise<void> {
    if (!child && revision !== undefined) throw new NativeRecordError('Managed root records are immutable')
    const bytes = Buffer.from(text)
    if (bytes.length > MAX_RECORD) throw new NativeRecordError('Provisioning record exceeds its bound')
    await this.call({ op: 'begin', child: child ?? null, leaf: leaf(child), revision: revision ?? null })
    for (let offset = 0; offset < bytes.length; offset += 8192) {
      await this.call({ op: 'chunk', offset, hex: bytes.subarray(offset, offset + 8192).toString('hex') })
    }
    await this.call({ op: 'commit' })
  }
}
function leaf(child?: string): string { return child ? '.substitute-provisioning.json' : '.substitute-managed.json' }
function samePath(left: string, right: string): boolean { return process.platform === 'win32' ? left.toLowerCase() === right.toLowerCase() : left === right }
let owner: NativeRecords | undefined
/** The worker bootstrap binds its sole authenticated host transport. */
export function bindNativeRecords(records: NativeRecords): void { owner = records }
/** Features consume the bound owner; filesystem access has no ambient fallback. */
export function nativeRecords(): NativeRecords {
  if (!owner) throw new NativeRecordError('Native record owner is unavailable')
  return owner
}
