// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Bound bursty upstream output while retaining the newest progress and every terminal event. */
export class Events {
  #write
  #progress
  #diagnostics = []
  #omitted = 0
  constructor(write) { this.#write = write }
  send(id, event, data) {
    if (event === 'progress') this.#progress = { id, event, data }
    else if (event === 'diagnostic') {
      if (this.#diagnostics.length === 8) { this.#diagnostics.shift(); this.#omitted++ }
      this.#diagnostics.push({ id, event, data })
    } else {
      this.flush()
      this.#write(id, event, data)
    }
  }
  flush() {
    if (this.#diagnostics.length) {
      const last = this.#diagnostics.at(-1)
      this.#write(last.id, 'diagnostic', {
        message: this.#diagnostics.map(item => String(item.data.message ?? JSON.stringify(item.data)).slice(-4096)).join('\n'),
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
