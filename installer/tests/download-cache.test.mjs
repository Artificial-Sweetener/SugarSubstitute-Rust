// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import { EventEmitter } from 'node:events'
import { build } from 'esbuild'
import { fileURLToPath } from 'node:url'

const artifact = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!artifact || !path.isAbsolute(artifact)) throw new Error('Explicit test artifact directory required')
const worker = fileURLToPath(new URL('../worker', import.meta.url))
const receipt = JSON.parse(fs.readFileSync(path.join(artifact, 'worker/build-receipt.json'), 'utf8'))
const input = receipt.inputs.find(file => file.endsWith('/src/main/lib/download.ts'))
const upstream = input.slice(0, input.indexOf('/src/main/'))

/** Real downloader, stream and client; only Electron networking and the external native port are fake. */
async function fixture(options = {}) {
  const directory = fs.mkdtempSync(path.join(artifact, 'native-download-'))
  const root = path.join(directory, 'cache'), destination = path.join(root, 'archive')
  const operations = [], files = new Map(), held = new Map()
  if (options.prefix) {
    files.set('archive', Buffer.from(options.prefix))
    files.set('archive.dl-meta', Buffer.from(JSON.stringify({ url: 'https://example.test/archive', expectedSize: 7, etag: 'original' })))
  }
  const state = { chunks: [Buffer.alloc(16)], size: 16, status: 200, end: true, ...options }
  const request = new EventEmitter(), response = new EventEmitter(), headers = new Map()
  request.setHeader = (name, value) => headers.set(name, value)
  request.abort = () => { state.aborted = true }
  request.end = () => queueMicrotask(() => {
    response.statusCode = state.status
    response.headers = { 'content-length': String(state.size), etag: 'original' }
    request.emit('response', response)
    for (const chunk of state.chunks) response.emit('data', chunk)
    if (state.end) response.emit('end')
  })
  const binding = '__native_download_' + Math.random().toString(16).slice(2)
  globalThis[binding] = { request, external: options.external === true }
  const { outputFiles } = await build({
    stdin: { contents: "export * from './downloader'; export * from './downloads'", resolveDir: worker, loader: 'ts' },
    bundle: true, write: false, platform: 'node', format: 'esm', target: 'node22',
    plugins: [{ name: 'external-boundaries', setup(builder) {
      builder.onResolve({ filter: /^@upstream\// }, args => ({ path: path.join(upstream, 'src/main', args.path.slice(10) + '.ts') }))
      builder.onResolve({ filter: /^electron$/ }, () => ({ path: 'electron', namespace: 'network-fixture' }))
      builder.onLoad({ filter: /.*/, namespace: 'network-fixture' }, () => ({ loader: 'js', contents: `export const net={request(){return globalThis[${JSON.stringify(binding)}].request}}` }))
      builder.onResolve({ filter: /settings$/ }, () => ({ path: path.join(worker, 'settings.ts') }))
      builder.onResolve({ filter: /^fs$/ }, () => ({ path: 'fs', namespace: 'forbidden-cache-filesystem' }))
      builder.onLoad({ filter: /.*/, namespace: 'forbidden-cache-filesystem' }, () => ({ loader: 'js', contents: `
        import fs from 'node:fs'
        export default globalThis[${JSON.stringify(binding)}].external ? fs : new Proxy({}, {
          get(_target,key){throw new Error('Cache filesystem bypass: '+String(key))}
        })
      ` }))
    } }],
  })
  const module = await import('data:text/javascript;base64,' + Buffer.from(outputFiles[0].text).toString('base64'))
  const client = new module.NativeDownloads((id, data) => {
    const operation = data.operation
    operations.push(operation)
    let result = { ok: true, data: {} }
    const bytes = files.get(operation.name)
    if (operation.op === 'length') result.data = { length: bytes?.length ?? null }
    if (operation.op === 'read') result = bytes ? { ok: true, data: { hex: bytes.toString('hex') } } : { ok: false, error: { code: 'ENOENT', osCode: 2 } }
    if (operation.op === 'write') files.set(operation.name, Buffer.from(operation.hex, 'hex'))
    if (operation.op === 'remove') files.delete(operation.name)
    if (operation.op === 'open') { if (!bytes) files.set(operation.name, Buffer.alloc(0)); result.data = { length: bytes?.length ?? 0 } }
    if (operation.op === 'reset') files.set(operation.name, Buffer.alloc(0))
    if (operation.op === 'chunk') {
      assert.equal(operation.offset, bytes.length)
      files.set(operation.name, Buffer.concat([bytes, Buffer.from(operation.hex, 'hex')]))
    }
    const reply = () => client.accept({ id, input: { serial: data.serial, index: 0, done: true, result } })
    const hold = held.get(operation.op)
    if (hold) { held.delete(operation.op); hold.reply = reply; hold.entered() }
    else queueMicrotask(reply)
  }, root)
  client.begin('original')
  module.bindNativeDownloads(client)
  return { module, operations, files, destination, root, directory, response, request, state, headers,
    hold(op) {
      const hold = {}; const entered = new Promise(resolve => { hold.entered = resolve })
      held.set(op, hold)
      return { entered, release() { hold.reply() } }
    }, dispose() { delete globalThis[binding] },
  }
}

test('native cache writes reject source-queue overflow before writes and preserve a resumable sidecar', async () => {
  const f = await fixture({ chunks: [Buffer.alloc(1024 * 1024), Buffer.alloc(16)], size: 1024 * 1024 + 16 })
  try {
    await assert.rejects(f.module.download('https://example.test/archive', f.destination, null), /write queue capacity/)
    assert.equal(f.operations.filter(op => op.op === 'chunk').length, 0)
    assert.equal(f.files.get('archive').length, 0)
    assert.equal(JSON.parse(f.files.get('archive.dl-meta')).expectedSize, 1024 * 1024 + 16)
    assert.equal(f.operations.at(-1).op, 'close')
  } finally { f.dispose() }
})

test('the existing downloader commits exact native bytes without unsupported response methods or filesystem access', async () => {
  const f = await fixture()
  try {
    assert.equal(await f.module.download('https://example.test/archive', f.destination, null), f.destination)
    assert.deepEqual(f.files.get('archive'), f.state.chunks[0])
    assert.equal(f.files.has('archive.dl-meta'), false)
    assert.equal(f.response.pause, undefined)
    assert.equal(fs.existsSync(f.root), false)
  } finally { f.dispose() }
})

test('native resume preserves Range and If-Range and appends only after validating the physical prefix', async () => {
  const f = await fixture({ prefix: 'abc', chunks: [Buffer.from('defg')], size: 4, status: 206 })
  try {
    await f.module.download('https://example.test/archive', f.destination, null, { expectedSize: 7 })
    assert.equal(f.headers.get('Range'), 'bytes=3-')
    assert.equal(f.headers.get('If-Range'), 'original')
    assert.equal(f.files.get('archive').toString(), 'abcdefg')
    assert.equal(f.operations.some(op => op.op === 'reset'), false)
    assert.equal(f.operations.find(op => op.op === 'chunk').offset, 3)
  } finally { f.dispose() }
})

test('a server returning a full body to a range request explicitly resets native bytes', async () => {
  const f = await fixture({ prefix: 'old', chunks: [Buffer.from('newdata')], size: 7 })
  try {
    await f.module.download('https://example.test/archive', f.destination, null)
    assert.equal(f.files.get('archive').toString(), 'newdata')
    assert.equal(f.operations.filter(op => op.op === 'reset').length, 1)
  } finally { f.dispose() }
})

test('early response bytes remain queued while native opening waits for its acknowledgement', async () => {
  const f = await fixture(), opening = f.hold('open')
  try {
    const downloading = f.module.download('https://example.test/archive', f.destination, null)
    await opening.entered
    assert.equal(f.operations.some(op => op.op === 'chunk'), false)
    opening.release()
    await downloading
    assert.equal(f.files.get('archive').length, 16)
  } finally { f.dispose() }
})

test('network failure waits for native prefix closure before rejection and retains resumable bytes', async () => {
  const f = await fixture({ chunks: [Buffer.from('abc')], size: 7, end: false })
  const writing = f.hold('chunk'), closing = f.hold('close')
  try {
    const cause = new Error('Network failed')
    let failure
    const downloading = f.module.download('https://example.test/archive', f.destination, null).catch(error => { failure = error })
    await writing.entered
    f.response.emit('error', cause)
    assert.equal(failure, undefined)
    writing.release()
    await closing.entered
    assert.equal(failure, undefined)
    closing.release()
    await downloading
    assert.equal(failure, cause)
    assert.equal(f.files.get('archive').toString(), 'abc')
    assert.equal(f.files.has('archive.dl-meta'), true)
  } finally { f.dispose() }
})

test('authored destinations retain the upstream filesystem path without native cache operations', async () => {
  const f = await fixture({ external: true })
  try {
    const destination = path.join(f.directory, 'authored', 'model')
    await f.module.download('https://example.test/model', destination, null)
    assert.deepEqual(fs.readFileSync(destination), f.state.chunks[0])
    assert.equal(f.operations.length, 0)
  } finally { f.dispose() }
})
