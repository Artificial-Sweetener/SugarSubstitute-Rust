// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Bound bursty upstream output while retaining the newest progress and every terminal event. */
import { diagnosticText } from './diagnostics.mjs'
import contract from '../../crates/sugarsubstitute-contracts/src/diagnostics/current.json' with { type: 'json' }
export class Events {
  #write
  #progress
  #diagnostics = []
  #omitted = 0
  constructor(write) { this.#write = write }
  send(id, event, data) {
    if (event === 'progress') this.#progress = { id, event, data }
    else if (event === 'diagnostic') {
      if (this.#diagnostics.length && this.#diagnostics.at(-1).id !== id) this.flush()
      if (this.#diagnostics.length === contract.worker_lines) { this.#diagnostics.shift(); this.#omitted = Math.min(Number.MAX_SAFE_INTEGER, this.#omitted + 1) }
      this.#diagnostics.push({ id, message: diagnosticText(data?.message) })
    } else {
      this.flush()
      this.#write(id, event, data)
    }
  }
  flush() {
    if (this.#diagnostics.length) {
      const last = this.#diagnostics.at(-1)
      this.#write(last.id, 'diagnostic', {
        message: this.#diagnostics.map(item => item.message).join('\n'),
        omitted: this.#omitted,
      })
      this.#diagnostics = []
      this.#omitted = 0
    }
    if (this.#progress) {
      const { id, event, data } = this.#progress
      this.#progress = undefined
      this.#write(id, event, data)
    }
  }
}
