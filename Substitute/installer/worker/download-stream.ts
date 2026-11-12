// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Adapt upstream's bounded Node write queue to the authenticated native payload lifecycle. */
import { Writable } from 'node:stream'
import type { NativeDownloads, DownloadFile } from './downloads'
import { DownloadStorageError } from './downloads'

/** No descriptor escapes. Construction, writes and both final/destroy paths await native custody. */
export class NativeDownloadStream extends Writable {
  private file: DownloadFile | undefined
  constructor(private readonly storage: NativeDownloads, private readonly destination: string,
    private readonly append: boolean, private readonly expected: number,
    private readonly beforeOpen: Promise<void>, private readonly prefix: number) {
    super({ autoDestroy: true, emitClose: true })
  }
  /** Node retains queued source buffers until the original sidecar and file preparation finish. */
  override _construct(callback: (error?: Error | null) => void): void {
    void this.prepare().then(() => callback(), cause => callback(asError(cause)))
  }
  private async prepare(): Promise<void> {
    await this.beforeOpen
    const file = await this.storage.open(this.destination, this.append, this.expected)
    this.file = file
    if (file.length !== this.prefix) throw new DownloadStorageError('Native payload prefix changed during response preparation', 'EBUSY')
  }
  /** The upstream writableLength guard still bounds source buffers while native replies are pending. */
  override _write(chunk: Buffer, _encoding: BufferEncoding, callback: (error?: Error | null) => void): void {
    if (!this.file) { callback(new Error('Native payload was not prepared')); return }
    void this.file.write(chunk).then(() => callback(), cause => callback(asError(cause)))
  }
  override _final(callback: (error?: Error | null) => void): void {
    void this.retire().then(() => callback(), cause => callback(asError(cause)))
  }
  /** Node waits for construction before destruction, and native close queues after admitted writes. */
  override _destroy(cause: Error | null, callback: (error?: Error | null) => void): void {
    void this.retire().then(() => callback(cause), cleanup => callback(cause
      ? new AggregateError([cause, cleanup], 'Native payload failed and could not retire') : asError(cleanup)))
  }
  private async retire(): Promise<void> { await this.file?.settled() }
  /** Match upstream's stream-close contract; the callback follows actual native prefix flush/close. */
  close(callback: () => void): void {
    if (this.closed) { queueMicrotask(callback); return }
    this.once('close', callback)
    this.destroy()
  }
}
function asError(cause: unknown): Error { return cause instanceof Error ? cause : new Error(String(cause)) }
