// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Local real-socket qualification; each server and destination belongs to one test. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { createServer as secureServer } from 'node:https'
import { connect } from 'node:net'
import certificate from './tls-fixture.mjs'
import { once } from 'node:events'
import { createRequire } from 'node:module'
import fs from 'node:fs/promises'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { runtimeFlags } from '../runtime.mjs'
const { download, downloadMetaPath, fetchJSON, connectProxyHost, receiveProxyResult, closeProxyHost, createRequest, systemRoutes, initializeMetadata } = createRequire(import.meta.url)(process.argv[2])
let resolveRoutes = () => [{ kind: 'direct' }]
let observeProxy = () => {}
connectProxyHost((id, event, data) => {
  observeProxy(id, event, data)
  if (event === 'proxy_resolve') {
    const routes = resolveRoutes(data.url)
    if (routes) receiveProxyResult(id, { routes })
  }
})
const workspace = process.argv[4]
await initializeMetadata()
const data = Buffer.alloc(512 * 1024, 37)

async function serve(t, handler) {
  const server = createServer(handler)
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  t.after(() => { server.closeAllConnections(); server.close() })
  return `http://127.0.0.1:${server.address().port}`
}

test('redirected downloads finish exact bytes and resume with Range and If-Range', { timeout: 10000 }, async t => {
  let range, validator
  const base = await serve(t, (req, res) => {
    if (req.url === '/redirect') { res.writeHead(302, { Location: '/file' }); res.end(); return }
    range = req.headers.range; validator = req.headers['if-range']
    const offset = range ? Number(range.match(/bytes=(\d+)-/)[1]) : 0
    res.writeHead(offset ? 206 : 200, { 'Content-Length': data.length - offset, ETag: '"fixture"', ...(offset ? { 'Content-Range': `bytes ${offset}-${data.length - 1}/${data.length}` } : {}) })
    res.end(data.subarray(offset))
  })
  const file = path.join(workspace, 'redirect.bin')
  await download(`${base}/redirect`, file, null, { expectedSize: data.length })
  assert.deepEqual(await fs.readFile(file), data)
  await assert.rejects(fs.stat(downloadMetaPath(file)), { code: 'ENOENT' })
  await fs.writeFile(file, data.subarray(0, 12345))
  await fs.writeFile(downloadMetaPath(file), JSON.stringify({ url: `${base}/file`, expectedSize: data.length, etag: '"fixture"' }))
  await download(`${base}/file`, file, null, { expectedSize: data.length })
  assert.equal(range, 'bytes=12345-'); assert.equal(validator, '"fixture"')
  assert.deepEqual(await fs.readFile(file), data)
})

test('ignored resume ranges restart cleanly and mismatched lengths reject', { timeout: 10000 }, async t => {
  const base = await serve(t, (_req, res) => { res.writeHead(200, { 'Content-Length': data.length }); res.end(data) })
  const file = path.join(workspace, 'restart.bin')
  await fs.writeFile(file, data.subarray(0, 1000))
  await fs.writeFile(downloadMetaPath(file), JSON.stringify({ url: base, expectedSize: data.length, etag: '"old"' }))
  await download(base, file, null, { expectedSize: data.length })
  assert.deepEqual(await fs.readFile(file), data)
  await assert.rejects(download(base, path.join(workspace, 'bad-size.bin'), null, { expectedSize: 4 }), /size|Size/)
})

test('cancellation settles a streaming download and keeps its resumable sidecar', { timeout: 10000 }, async t => {
  const base = await serve(t, (_req, res) => {
    res.writeHead(200, { 'Content-Length': data.length, ETag: '"fixture"' })
    res.write(data.subarray(0, 16384))
  })
  const control = new AbortController(), file = path.join(workspace, 'cancel.bin')
  await assert.rejects(download(base, file, () => control.abort(), { signal: control.signal, expectedSize: data.length }), /cancelled/)
  const meta = JSON.parse(await fs.readFile(downloadMetaPath(file), 'utf8'))
  assert.equal(meta.expectedSize, data.length)
  assert.equal(meta.etag, '"fixture"')
})

test('cancellation before response headers retires the request and its watchdog', { timeout: 10000 }, async t => {
  const control = new AbortController()
  const base = await serve(t, () => control.abort())
  await assert.rejects(download(base, path.join(workspace, 'before-headers.bin'), null, { signal: control.signal }), /cancelled/)
})

test('metadata follows relative redirects, preserves Unicode and negotiates ETags', { timeout: 10000 }, async t => {
  let cached = false
  const base = await serve(t, (req, res) => {
    assert.equal(req.headers.authorization, undefined)
    if (req.url === '/redirect') { res.writeHead(302, { Location: '/json' }); res.end(); return }
    if (req.headers['if-none-match'] === '"json"') { cached = true; res.writeHead(304); res.end(); return }
    res.writeHead(200, { ETag: '"json"' }); res.end(JSON.stringify({ text: '日本語 🍬' }))
  })
  assert.deepEqual(await fetchJSON(`${base}/redirect`), { text: '日本語 🍬' })
  assert.deepEqual(await fetchJSON(`${base}/json`), { text: '日本語 🍬' })
  assert.deepEqual(await fetchJSON(`${base}/json`), { text: '日本語 🍬' })
  assert.ok(cached)
})

test('metadata rejects redirect loops, malformed JSON and oversized bodies', { timeout: 10000 }, async t => {
  const base = await serve(t, (req, res) => {
    if (req.url === '/loop') { res.writeHead(302, { Location: '/loop' }); res.end(); return }
    res.end(req.url === '/huge' ? 'x'.repeat(8 * 1024 * 1024 + 1) : '{')
  })
  await assert.rejects(fetchJSON(`${base}/loop`), /redirect/)
  await assert.rejects(fetchJSON(`${base}/invalid`), /Invalid JSON/)
  await assert.rejects(fetchJSON(`${base}/huge`), /bound/)
})

test('the shipped runtime honors an explicitly configured environment proxy', { timeout: 10000 }, async t => {
  let requested
  const proxy = await serve(t, (req, res) => {
    requested = req.url
    res.end(JSON.stringify({ throughProxy: true }))
  })
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !/^(?:https?_proxy|all_proxy|no_proxy|node_options|node_test_context)$/i.test(name)))
  env.HTTP_PROXY = proxy
  env.NO_PROXY = ''
  const source = 'import { createRequire } from "node:module"; const { fetchJSON, initializeMetadata } = createRequire(import.meta.url)(process.argv[1]); await initializeMetadata(); const result = await fetchJSON("http://substitute-proxy-fixture.invalid/metadata"); if (result.throughProxy !== true) throw new Error("Proxy not used");'
  await promisify(execFile)(process.execPath, [...runtimeFlags, '--input-type=module', '-e', source, process.argv[2], '--setup-workspace', workspace, '--setup-cache-metadata', path.join(workspace, 'metadata-child')], {
    env, windowsHide: true, timeout: 8000,
  })
  assert.equal(requested, 'http://substitute-proxy-fixture.invalid/metadata')
})

test('system proxy routes are resolved per redirect and preserve ordered failover', { timeout: 10000 }, async t => {
  const resolved = [], forwarded = []
  const broken = await serve(t, (request) => request.socket.destroy())
  const proxy = await serve(t, (request, response) => {
    forwarded.push(request.url)
    if (request.url.endsWith('/start')) { response.writeHead(302, { Location: 'http://second-proxy-fixture.invalid/end' }); response.end() }
    else response.end('{"proxied":true}')
  })
  resolveRoutes = url => { resolved.push(url); return [{ kind: 'proxy', url: broken }, { kind: 'proxy', url: proxy }] }
  t.after(() => { resolveRoutes = () => [{ kind: 'direct' }] })
  assert.deepEqual(await fetchJSON('http://first-proxy-fixture.invalid/start'), { proxied: true })
  assert.deepEqual(resolved, ['http://first-proxy-fixture.invalid/start', 'http://second-proxy-fixture.invalid/end'])
  assert.deepEqual(forwarded, resolved)
})

test('a proxy failure cannot invent a direct fallback or bypass authentication', { timeout: 10000 }, async t => {
  let direct = 0
  const destination = await serve(t, (_request, response) => { direct++; response.end('{}') })
  const broken = await serve(t, request => request.socket.destroy())
  const auth = await serve(t, (_request, response) => { response.writeHead(407); response.end() })
  t.after(() => { resolveRoutes = () => [{ kind: 'direct' }] })
  resolveRoutes = () => [{ kind: 'proxy', url: broken }]
  await assert.rejects(fetchJSON(`${destination}/failure`))
  assert.equal(direct, 0)
  resolveRoutes = () => [{ kind: 'proxy', url: auth }, { kind: 'direct' }]
  await assert.rejects(fetchJSON(`${destination}/auth`), /407/)
  assert.equal(direct, 0)
  resolveRoutes = () => [{ kind: 'proxy', url: broken }, { kind: 'direct' }]
  assert.deepEqual(await fetchJSON(`${destination}/allowed-direct`), {})
  assert.equal(direct, 1)
})

test('cancelling discovery retires its correlation and ignores late replies', { timeout: 10000 }, async t => {
  let connections = 0, identity, cancelled = false
  const destination = await serve(t, (_request, response) => { connections++; response.end('{}') })
  const control = new AbortController()
  resolveRoutes = () => undefined
  observeProxy = (id, event) => {
    if (event === 'proxy_resolve') { identity = id; queueMicrotask(() => control.abort()) }
    if (event === 'proxy_cancel') cancelled = true
  }
  t.after(() => { resolveRoutes = () => [{ kind: 'direct' }]; observeProxy = () => {} })
  await assert.rejects(download(destination, path.join(workspace, 'cancel-discovery.bin'), null, { signal: control.signal }), /cancelled/)
  assert.equal(cancelled, true)
  receiveProxyResult(identity, { routes: [{ kind: 'direct' }] })
  assert.equal(connections, 0)
})

test('HTTPS downloads tunnel through HTTP and HTTPS proxies with certificate verification', { timeout: 15000 }, async t => {
  const destination = secureServer(certificate, (_request, response) => response.end('{"secure":true}'))
  destination.listen(0, '127.0.0.1')
  await once(destination, 'listening')
  t.after(() => { destination.closeAllConnections(); destination.close() })
  const ca = path.join(workspace, 'fixture-ca.pem')
  await fs.writeFile(ca, certificate.cert)
  for (const secure of [false, true]) {
    const sockets = new Set(), targets = []
    const proxy = secure ? secureServer(certificate) : createServer()
    proxy.on('connect', (request, socket, head) => {
      targets.push(request.url)
      const upstream = connect(destination.address().port, '127.0.0.1', () => {
        socket.write('HTTP/1.1 200 Connection Established\r\n\r\n')
        if (head.length) upstream.write(head)
        socket.pipe(upstream).pipe(socket)
      })
      sockets.add(socket); sockets.add(upstream)
      socket.on('error', () => upstream.destroy())
      upstream.on('error', () => socket.destroy())
      socket.on('close', () => { sockets.delete(socket); upstream.destroy() })
      upstream.on('close', () => { sockets.delete(upstream); socket.destroy() })
    })
    proxy.listen(0, '127.0.0.1')
    await once(proxy, 'listening')
    t.after(() => { for (const socket of sockets) socket.destroy(); proxy.close() })
    const route = `${secure ? 'https' : 'http'}://127.0.0.1:${proxy.address().port}`
    const target = `https://127.0.0.1:${destination.address().port}/metadata`
    const source = 'import { createRequire } from "node:module"; const worker=createRequire(import.meta.url)(process.argv[1]); await worker.initializeMetadata(); worker.connectProxyHost((id,event)=>{if(event==="proxy_resolve")worker.receiveProxyResult(id,{routes:[{kind:"proxy",url:process.argv[2]}]})}); const value=await worker.fetchJSON(process.argv[3]); if(!value.secure)throw new Error("Secure tunnel failed"); worker.closeProxyHost();'
    const env = { ...process.env, NODE_EXTRA_CA_CERTS: ca }
    delete env.NODE_TEST_CONTEXT
    await promisify(execFile)(process.execPath, [...runtimeFlags, '--input-type=module', '-e', source, process.argv[2], route, target, '--setup-workspace', workspace, '--setup-cache-metadata', path.join(workspace, 'metadata-tunnel')], { env, windowsHide: true, timeout: 8000 })
    assert.deepEqual(targets, [`127.0.0.1:${destination.address().port}`])
  }
})

test('host disconnect cancels active sockets, rejects pending lookups and prevents new work', async t => {
  let connected
  const started = new Promise(resolve => { connected = resolve })
  const destination = await serve(t, () => connected())
  const downloadResult = fetchJSON(`${destination}/host-closure`)
  const rejected = assert.rejects(downloadResult)
  await started
  resolveRoutes = () => undefined
  const control = new AbortController()
  const pending = Array.from({ length: 8 }, () => systemRoutes('https://target.invalid/', control.signal))
  await assert.rejects(systemRoutes('https://target.invalid/', control.signal), /capacity/)
  const completed = Promise.all(pending.map(value => assert.rejects(value, /disconnected/)))
  closeProxyHost()
  await rejected
  await completed
  await assert.rejects(systemRoutes('https://target.invalid/', control.signal), /unavailable/)
})
