// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import fs from 'node:fs'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { parseCommand } from '../worker/protocol.mjs'
const { outputFiles } = await build({ stdin: { contents: "export * from './metadata'; export { bindDiagnostics } from './diagnostics.mjs';", resolveDir: fileURLToPath(new URL('../worker', import.meta.url)), loader: 'ts' }, bundle: true, write: false, platform: 'node', format: 'esm' })
const { NativeMetadata, readFileSafe, bindDiagnostics } = await import('data:text/javascript;base64,' + Buffer.from(outputFiles[0].text).toString('base64'))
const diagnostics = []
bindDiagnostics((level, message) => diagnostics.push({ level, message }))
const root = path.join(process.env.SUBSTITUTE_INSTALLER_BUILD, 'native-metadata')
/** Pure external native-port fake; actual file, grant and retirement mechanics are tested in Rust. */
function host(initial = {}) {
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
test('native metadata preserves multichunk Unicode and synchronous upstream snapshots after publication', async () => {
  const text = '糖😀'.repeat(3000), h = host({ 'fetch-cache.json': text })
  assert.throws(() => h.client.readFile(path.join(root, 'fetch-cache.json')), /not loaded/)
  await h.client.load()
  assert.equal(h.client.readFile(path.join(root, 'fetch-cache.json')), text)
  const next = text + 'updated'
  await h.client.writeFile(path.join(root, 'fetch-cache.json'), next)
  assert.equal(h.files.get('fetch-cache.json').toString(), next)
  assert.equal(h.client.readFile(path.join(root, 'fetch-cache.json')), next)
  assert.ok(h.operations.some(op => op.op === 'next'))
  assert.ok(h.operations.filter(op => op.op === 'chunk').length > 1)
})
test('missing transport and foreign metadata files cannot fall back to a local filesystem', async () => {
  assert.throws(() => readFileSafe(path.join(root, 'fetch-cache.json')), /absent/)
  const h = host()
  for (const file of [path.join(root, 'settings.json'), path.join(root, '..', 'fetch-cache.json'), 'fetch-cache.json']) {
    assert.throws(() => h.client.readFile(file), /Foreign/)
    await assert.rejects(h.client.writeFile(file, 'bad'), /Foreign/)
  }
  assert.equal(h.operations.length, 0)
})
test('native pressure preserves committed snapshots and exposes the existing localized cache failure', async () => {
  const h = host({ 'fetch-cache.json': 'original' })
  await h.client.load(); h.pressure()
  await assert.rejects(h.client.writeFile(path.join(root, 'fetch-cache.json'), 'new'), error => error.statusCode === 'cacheFull' && error.osCode === 112 && error.cause.code === 'ENOSPC')
  assert.equal(h.client.readFile(path.join(root, 'fetch-cache.json')), 'original')
  assert.equal(h.files.get('fetch-cache.json').toString(), 'original')
})
test('complete transfers serialize and native errors never turn into unobserved local publication', async () => {
  const h = host()
  await Promise.all(['one', 'two', 'three'].map(text => h.client.writeFile(path.join(root, 'fetch-cache.json'), text)))
  await h.client.drain()
  assert.equal(h.files.get('fetch-cache.json').toString(), 'three')
  assert.deepEqual(h.operations.filter(op => op.op === 'begin').map(op => op.length), [3, 3, 5])
})
test('transport closure rejects the actual pending request and queued operations retain their generation', async () => {
  const client = new NativeMetadata(() => {}, root)
  client.begin('original')
  const pending = client.writeFile(path.join(root, 'fetch-cache.json'), 'one')
  const queued = client.writeFile(path.join(root, 'fetch-cache.json'), 'two')
  const failures = [assert.rejects(pending, /closed/), assert.rejects(queued, /retired/)]
  await Promise.resolve()
  client.close()
  await Promise.all(failures)
  await client.drain()
})
test('metadata replies accept disk pressure only on the metadata subprotocol', () => {
  const command = { version: 1, id: 'original', operation: 'metadata_reply', input: {
    version: 1, serial: 1, index: 0, done: true, result: { ok: false, error: { code: 'ENOSPC', osCode: 112 } },
  } }
  assert.equal(parseCommand(JSON.stringify(command)).input.result.error.code, 'ENOSPC')
  assert.throws(() => parseCommand(JSON.stringify({ ...command, operation: 'storage_reply' })))
})
test('the bounded transfer queue rejects overload and cannot be rebound while original work is pending', async () => {
  const sent = [], client = new NativeMetadata((id, data) => sent.push({ id, data }), root)
  client.begin('original')
  const pending = Array.from({ length: 8 }, () => client.writeFile(path.join(root, 'fetch-cache.json'), 'a'))
  const failures = pending.map(task => assert.rejects(task))
  await assert.rejects(client.writeFile(path.join(root, 'fetch-cache.json'), 'ninth'), /full/)
  assert.throws(() => client.begin('foreign'), /queued/)
  client.close()
  await Promise.all(failures)
  await client.drain()
  assert.ok(sent.length <= 1)
  client.begin('next')
})
test('actual patched upstream ETag reads use the native snapshot and refresh waits for native publication', async () => {
  const receipt = JSON.parse(fs.readFileSync(path.join(process.env.SUBSTITUTE_INSTALLER_BUILD, 'worker/build-receipt.json'), 'utf8'))
  const source = receipt.inputs.find(input => input.endsWith('/src/main/lib/fetch.ts'))
  assert.ok(source)
  const metadata = fileURLToPath(new URL('../worker/metadata.ts', import.meta.url)).replaceAll('\\', '/')
  const { outputFiles } = await build({ stdin: { contents: `export { fetchJSON } from ${JSON.stringify(source)}; export { bindNativeMetadata } from ${JSON.stringify(metadata)}; export { requests } from 'electron';`, resolveDir: path.dirname(source) }, bundle: true, write: false, platform: 'node', format: 'esm', plugins: [{
    name: 'network-boundary', setup(builder) {
      builder.onResolve({ filter: /^electron$/ }, () => ({ path: 'electron', namespace: 'network-fixture' }))
      builder.onLoad({ filter: /.*/, namespace: 'network-fixture' }, () => ({ contents: `
        import { EventEmitter } from 'node:events'; export const requests = [];
        export const net = { request() {
          const request = new EventEmitter(); request.headers = {};
          request.setHeader = (name, value) => request.headers[name] = value;
          request.end = () => queueMicrotask(() => {
            const response = new EventEmitter(); response.statusCode = requests.length === 1 ? 304 : 200;
            response.headers = { etag: 'updated' }; request.emit('response', response);
            if (response.statusCode === 200) response.emit('data', Buffer.from('{"updated":true}'));
            response.emit('end');
          }); requests.push(request); return request;
        } };`, loader: 'js' }))
      builder.onResolve({ filter: /^\.\/paths$/ }, () => ({ path: 'paths', namespace: 'cache-root-fixture' }))
      builder.onLoad({ filter: /.*/, namespace: 'cache-root-fixture' }, () => ({ contents: `export const cacheDir = () => ${JSON.stringify(root)};`, loader: 'js' }))
      builder.onResolve({ filter: /^\.\/r2Mirror$/ }, () => ({ path: 'mirror', namespace: 'mirror-fixture' }))
      builder.onLoad({ filter: /.*/, namespace: 'mirror-fixture' }, () => ({ contents: 'export const r2MirrorUrl = url => url;', loader: 'js' }))
      builder.onResolve({ filter: /^\.\/native-metadata$/ }, () => ({ path: metadata }))
    },
  }] })
  const upstream = await import('data:text/javascript;base64,' + Buffer.from(outputFiles[0].text).toString('base64'))
  const url = 'https://example.invalid/metadata'
  const h = host({ 'fetch-cache.json': JSON.stringify({ [url]: { etag: 'original', data: { cached: true } } }) })
  await h.client.load(); upstream.bindNativeMetadata(h.client)
  assert.deepEqual(await upstream.fetchJSON(url), { cached: true })
  assert.equal(upstream.requests[0].headers['If-None-Match'], 'original')
  const gate = h.holdCommit()
  let settled = false
  const refreshed = upstream.fetchJSON(url, { refresh: true }).then(value => { settled = true; return value })
  await gate.entered
  assert.equal(settled, false)
  gate.release()
  assert.deepEqual(await refreshed, { updated: true })
  assert.equal(JSON.parse(h.files.get('fetch-cache.json').toString())[url].etag, 'updated')
})
