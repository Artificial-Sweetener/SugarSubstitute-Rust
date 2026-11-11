// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Ordered private native exchanges shared by record and cache storage clients. */
export type Data = Record<string, unknown>
type Pending = { serial: number; frames: Data[]; resolve: (data: Data[]) => void; reject: (cause: Error) => void }

/** One in-flight exchange, owned by the current provisioning command and its transport. */
export class NativeExchange {
  private id: string | undefined
  private serial = 0
  private pending: Pending | undefined
  constructor(private readonly send: (id: string, data: Data) => void, private readonly failure: (message: string) => Error) {}

  /** A host command starts a fresh sequence; cancellation preserves its recovery-write scope. */
  begin(id: string): void {
    if (this.pending) throw this.failure('Native storage exchange is still active')
    this.id = id; this.serial = 0
  }
  /** Transport closure rejects the actual pending exchange; it grants no filesystem completion. */
  close(cause: Error = this.failure('Native storage transport closed')): void {
    const pending = this.pending
    this.pending = undefined; this.id = undefined
    pending?.reject(cause)
  }
  /** Consume only a matching ordered reply. Duplicate, foreign and excessive frames fail closed. */
  accept(command: { id: string; input: { serial: number; index: number; done: boolean; result: any } }): void {
    const pending = this.pending
    const { serial, index, done, result } = command.input
    if (!pending || command.id !== this.id || serial !== pending.serial || index !== pending.frames.length || index >= 8) {
      throw this.failure('Native storage reply changed identity or sequence')
    }
    if (result.ok !== true) {
      if (index !== 0 || !done) throw this.failure('Native storage failure was fragmented')
      this.pending = undefined
      const cause = Object.assign(this.failure('Native storage operation failed'), result.error)
      pending.reject(cause)
      return
    }
    pending.frames.push(result.data)
    if (done) { this.pending = undefined; pending.resolve(pending.frames) }
  }
  call(operation: Data): Promise<Data[]> {
    if (!this.id || this.pending || this.serial >= Number.MAX_SAFE_INTEGER) {
      return Promise.reject(this.failure('Native storage scope is unavailable or busy'))
    }
    const id = this.id
    const serial = ++this.serial
    return new Promise((resolve, reject) => {
      this.pending = { serial, frames: [], resolve, reject }
      try { this.send(id, { version: 1, serial, operation }) }
      catch (cause) { this.pending = undefined; reject(cause) }
    })
  }
}
