// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import assert from 'node:assert/strict'
/** Pure external native-port fake; actual file, grant and retirement mechanics are tested in Rust. */
export function createMetadataHost(NativeMetadata, root, initial = {}) {
  const files = new Map(Object.entries(initial).map(([name, value]) => [name, Buffer.from(value)]))
  const operations = []
  let reading, writing, pressure = false, hold = false, release, entered
  const committing = new Promise(resolve => { entered = resolve })
  const client = new NativeMetadata((id, data) => {
    const request = data.operation
    operations.push(request)
    let result = { ok: true, data: {} }
    if (request.op === 'read') {
      const bytes = files.get(request.name)
      if (!bytes) result = { ok: false, error: { code: 'ENOENT', osCode: 2 } }
      else reading = Buffer.from(bytes)
    }
    if ((request.op === 'read' && result.ok) || request.op === 'next') {
      result.data = { hex: reading.subarray(0, 8192).toString('hex'), done: reading.length <= 8192 }
      reading = reading.subarray(8192)
    }
    if (request.op === 'begin') {
      if (pressure) result = { ok: false, error: { code: 'ENOSPC', osCode: 112 } }
      else writing = { name: request.name, length: request.length, bytes: Buffer.alloc(0) }
    }
    if (request.op === 'chunk') {
      assert.equal(request.offset, writing.bytes.length)
      assert.ok(request.hex.length <= 16384)
      writing.bytes = Buffer.concat([writing.bytes, Buffer.from(request.hex, 'hex')])
    }
    if (request.op === 'commit') {
      assert.equal(writing.bytes.length, writing.length)
      files.set(writing.name, writing.bytes); writing = undefined
    }
    if (request.op === 'abort') { reading = undefined; writing = undefined }
    const reply = () => client.accept({ id, input: { serial: data.serial, index: 0, done: true, result } })
    if (hold && request.op === 'commit') { release = reply; entered() }
    else queueMicrotask(reply)
  }, root)
  client.begin('original')
  return { client, files, operations, pressure: () => { pressure = true }, holdCommit: () => {
    hold = true; return { entered: committing, release: () => release() }
  } }
}