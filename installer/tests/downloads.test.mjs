// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { parseCommand } from '../worker/protocol.mjs'
import { finished } from 'node:stream/promises'
const { outputFiles } = await build({ entryPoints: [fileURLToPath(new URL('../worker/downloads.ts', import.meta.url))], bundle: true, write: false, platform: 'node', format: 'esm' })
const { NativeDownloads, DownloadStorageError } = await import('data:text/javascript;base64,' + Buffer.from(outputFiles[0].text).toString('base64'))
const streamBuild = await build({ entryPoints: [fileURLToPath(new URL('../worker/download-stream.ts', import.meta.url))], bundle: true, write: false, platform: 'node', format: 'esm' })
const { NativeDownloadStream } = await import('data:text/javascript;base64,' + Buffer.from(streamBuild.outputFiles[0].text).toString('base64'))
const root = path.join(process.env.SUBSTITUTE_INSTALLER_BUILD, 'native-downloads')

/** Script the external native reply port; Rust tests own actual files, byte grants and retirement. */
function host(reply = () => ({ ok: true, data: {} })) {
  const operations = [], waits = new Map()
  const client = new NativeDownloads((id, data) => {
    operations.push(data.operation)
    const result = reply(data.operation)
    const respond = () => client.accept({ id, input: { serial: data.serial, index: 0, done: true, result } })
    const wait = waits.get(data.operation.op)
    if (wait) { waits.delete(data.operation.op); wait.respond = respond; wait.entered() }
    else queueMicrotask(respond)
  }, root)
  client.begin('original')
  return { client, operations, hold(op) {
    const state = {}; const entered = new Promise(resolve => { state.entered = resolve })
    waits.set(op, state)
    return { entered, release() { state.respond() } }
  } }
}

test('download payload calls preserve physical offsets and bounded binary chunks before actual close', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 3 } : {} }))
  const file = await h.client.open(path.join(root, 'entry/payload'), true, 200000)
  const bytes = Buffer.from('糖😀'.repeat(15000))
  await file.write(bytes)
  await file.settled()
  const chunks = h.operations.filter(operation => operation.op === 'chunk')
  assert.ok(chunks.length > 1)
  let position = 3
  for (const chunk of chunks) {
    assert.equal(chunk.offset, position)
    assert.ok(chunk.hex.length <= 24 * 1024 * 2)
    assert.equal(chunk.name, 'entry/payload')
    position += chunk.hex.length / 2
  }
  assert.deepEqual(Buffer.concat(chunks.map(chunk => Buffer.from(chunk.hex, 'hex'))), bytes)
  assert.equal(h.operations.at(-1).op, 'close')
  await assert.rejects(file.write(Buffer.from('x')), /closed/)
})

test('every settlement observer waits for the same native close acknowledgement', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 0 } : {} }))
  const file = await h.client.open(path.join(root, 'payload'), false, 8)
  const held = h.hold('close')
  const first = file.settled(), second = file.settled()
  let observed = false
  void second.then(() => { observed = true })
  await held.entered
  assert.equal(observed, false)
  held.release()
  await Promise.all([first, second])
  assert.equal(observed, true)
  assert.equal(h.operations.filter(operation => operation.op === 'close').length, 1)
})

test('queued payload byte admission rejects overload before another native request is sent', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 0 } : {} }))
  const first = await h.client.open(path.join(root, 'first'), true)
  const second = await h.client.open(path.join(root, 'second'), true)
  const held = h.hold('chunk')
  const filling = first.write(Buffer.alloc(1024 * 1024, 1))
  await held.entered
  await assert.rejects(second.write(Buffer.from('x')), error => error.code === 'EBUSY')
  assert.equal(h.operations.filter(operation => operation.op === 'chunk').length, 1)
  held.release()
  await filling
  await Promise.all([first.settled(), second.settled()])
})

test('native pressure preserves error identity and only scoped reply operations admit ENOSPC', async () => {
  const h = host(() => ({ ok: false, error: { code: 'ENOSPC', osCode: 112 } }))
  await assert.rejects(h.client.writeFile(path.join(root, 'sidecar'), '{}'), error => {
    assert.ok(error instanceof DownloadStorageError)
    assert.equal(error.statusCode, 'cacheFull')
    assert.equal(error.code, 'ENOSPC')
    assert.equal(error.osCode, 112)
    assert.ok(error.cause)
    return true
  })
  const command = { version: 1, id: 'original', operation: 'downloads_reply', input: { version: 1, serial: 1, index: 0, done: true, result: { ok: false, error: { code: 'ENOSPC', osCode: 112 } } } }
  assert.equal(parseCommand(JSON.stringify(command)).operation, 'downloads_reply')
  assert.throws(() => parseCommand(JSON.stringify({ ...command, operation: 'storage_reply' })))
})

test('foreign selectors and changed native entry locations fail without filesystem fallback', async () => {
  const h = host(() => ({ ok: true, data: { path: path.join(root, '..', 'foreign') } }))
  await assert.rejects(h.client.entry('upstream'), /changed identity/)
  const before = h.operations.length
  await assert.rejects(h.client.readFile(path.join(root, '..', 'foreign')), /Foreign/)
  assert.equal(h.operations.length, before)
})

test('failed reset retains its cause and waits for native retirement before rejecting preparation', async () => {
  const h = host(operation => operation.op === 'reset'
    ? { ok: false, error: { code: 'EIO', osCode: 5 } }
    : { ok: true, data: operation.op === 'open' ? { length: 7 } : {} })
  const held = h.hold('close')
  let failure
  const attempt = h.client.open(path.join(root, 'payload'), false, 8).catch(cause => { failure = cause })
  await held.entered
  assert.equal(failure, undefined)
  held.release()
  await attempt
  assert.equal(failure.code, 'EIO')
  assert.equal(failure.osCode, 5)
  assert.deepEqual(h.operations.map(operation => operation.op), ['open', 'reset', 'close'])
})

test('the upstream writable queue retains early bytes until native preparation and actual close complete', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 0 } : {} }))
  let prepare
  const beforeOpen = new Promise(resolve => { prepare = resolve })
  const stream = new NativeDownloadStream(h.client, path.join(root, 'payload'), false, 3, beforeOpen, 0)
  const settled = finished(stream, { cleanup: true })
  stream.write(Buffer.from('abc'))
  stream.end()
  assert.equal(stream.writableLength, 3)
  assert.equal(h.operations.length, 0)
  prepare()
  await settled
  assert.deepEqual(h.operations.map(operation => operation.op), ['open', 'reset', 'chunk', 'close'])
  assert.equal(h.operations[2].hex, Buffer.from('abc').toString('hex'))
  assert.equal(stream.closed, true)
})

test('destroy waits for an admitted write and native prefix retirement before reporting stream closure', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 0 } : {} }))
  const writing = h.hold('chunk'), closing = h.hold('close')
  const stream = new NativeDownloadStream(h.client, path.join(root, 'payload'), false, 3, Promise.resolve(), 0)
  const cause = new Error('Network stopped')
  let observedError, observedClose = false
  stream.on('error', error => { observedError = error })
  const closed = new Promise(resolve => stream.once('close', () => { observedClose = true; resolve() }))
  stream.write(Buffer.from('abc'))
  await writing.entered
  stream.destroy(cause)
  assert.equal(observedClose, false)
  writing.release()
  await closing.entered
  assert.equal(observedClose, false)
  closing.release()
  await closed
  assert.equal(observedError, cause)
  assert.equal(h.operations.filter(operation => operation.op === 'close').length, 1)
})

test('destruction during native construction closes the acquired file without publishing queued body bytes', async () => {
  const h = host(operation => ({ ok: true, data: operation.op === 'open' ? { length: 0 } : {} }))
  const opening = h.hold('open')
  const stream = new NativeDownloadStream(h.client, path.join(root, 'payload'), false, 3, Promise.resolve(), 0)
  stream.on('error', () => {})
  const closed = new Promise(resolve => stream.once('close', resolve))
  stream.write(Buffer.from('abc'))
  await opening.entered
  stream.destroy(new Error('Network stopped'))
  assert.equal(stream.closed, false)
  opening.release()
  await closed
  assert.deepEqual(h.operations.map(operation => operation.op), ['open', 'reset', 'close'])
})
