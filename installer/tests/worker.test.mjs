// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import fs from 'node:fs/promises'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import readline from 'node:readline'
import { createServer } from 'node:net'
import { randomBytes } from 'node:crypto'

const build = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!build || !path.isAbsolute(build)) throw new Error('Build the worker and set SUBSTITUTE_INSTALLER_BUILD')

test('real headless worker rejects unsafe requests and exits on host closure', { timeout: 30000 }, async t => {
  const observed = []
  const workspace = await fs.mkdtemp(path.join(build, 'protocol-'))
  const downloads = path.join(workspace, 'downloads')
  const metadata = path.join(workspace, 'metadata')
  await fs.mkdir(downloads); await fs.mkdir(metadata)
  const runtime = path.join(workspace, 'runtime')
  await fs.mkdir(runtime)
  const executable = process.platform === 'win32' ? 'electron.exe' :
    process.platform === 'darwin' ? 'Electron.app/Contents/MacOS/Electron' : 'electron'
  const server = createServer()
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const connected = once(server, 'connection', { signal: t.signal })
  const token = randomBytes(32).toString('hex')
  const env = { ...process.env }
  delete env.ELECTRON_RUN_AS_NODE
  delete env.NODE_OPTIONS
  const child = spawn(path.join(build, 'electron', executable), [
    path.join(build, 'worker'), '--setup-workspace', workspace,
    '--setup-cache-runtime', runtime,
    '--setup-port', String(server.address().port), '--setup-token', token,
    '--setup-cache-downloads', downloads, '--setup-cache-download-bytes', String(24 * 1024 ** 3),
    '--setup-cache-metadata', metadata, '--setup-cache-metadata-bytes', String(256 * 1024 ** 2),
  ], {
    windowsHide: true,
    env,
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  const closed = once(child, 'close')
  let stderr = ''
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-8192) })
  child.stdout.resume()
  let socket, lines
  t.after(async () => {
    if (t.signal.aborted) t.diagnostic(JSON.stringify({ observed, stderr }))
    if (child.exitCode === null && child.signalCode === null) child.kill()
    socket?.destroy()
    server.close()
    lines?.close()
    await closed
  })
  observed.push('waiting for authenticated worker')
  ;[socket] = await Promise.race([
    connected,
    closed.then(([code, signal]) => {
      throw new Error(`Worker exited before connecting (${code}, ${signal}): ${stderr}`)
    }),
  ])
  observed.push('connected')
  server.close()
  lines = readline.createInterface({ input: socket, signal: t.signal })
  const iterator = lines[Symbol.asyncIterator]()
  async function nextTerminal(event) {
    for (;;) {
      const next = await iterator.next()
      if (next.done) {
        const [code, signal] = await closed
        assert.fail(`Worker exited early (${code}, ${signal}): ${stderr}`)
      }
      if (!next.value.trim()) continue
      const frame = JSON.parse(next.value)
      observed.push({ event: frame.event, id: frame.id })
      assert.equal(frame.version, 1)
      if (frame.event === 'metadata') {
        assert.ok(['read', 'abort'].includes(frame.data.operation.op))
        socket.write(JSON.stringify({ version: 1, id: frame.id, operation: 'metadata_reply', input: {
          version: 1, serial: frame.data.serial, index: 0, done: true,
          result: frame.data.operation.op === 'read' ? { ok: false, error: { code: 'ENOENT', osCode: 2 } } : { ok: true, data: {} },
        } }) + '\n')
        continue
      }
      if (frame.event === 'diagnostic') continue
      assert.equal(frame.event, event, JSON.stringify(frame))
      return frame
    }
  }
  const authentication = await iterator.next()
  assert.equal(JSON.parse(authentication.value).token, token)
  const ready = await nextTerminal('ready')
  assert.equal(ready.data.cacheProtocol, 2)
  assert.equal(ready.data.storageProtocol, 1)
  assert.equal(ready.data.metadataProtocol, 1)
  assert.equal(ready.data.downloadsProtocol, 1)
  assert.match(ready.data.upstreamRevision, /^[a-f0-9]{40}$/)
  assert.deepEqual(ready.data.operations, ['options', 'inspect', 'provision', 'integrate', 'cancel'])
  socket.write(JSON.stringify({
    version: 1, id: 'stale', operation: 'provision',
    input: { directory: path.join(workspace, 'must-not-exist'), selectionToken: 'unissued' },
  }) + '\n')
  const failure = await nextTerminal('failed')
  assert.equal(failure.id, 'stale')
  await assert.rejects(fs.access(path.join(workspace, 'must-not-exist')))
  const selected = path.join(workspace, 'selected-missing')
  socket.write(JSON.stringify({ version: 1, id: 'inspect-records', operation: 'inspect', input: { directory: selected } }) + '\n')
  const storage = await nextTerminal('storage')
  assert.equal(storage.id, 'inspect-records')
  assert.equal(storage.data.version, 1)
  assert.deepEqual(storage.data.operation, { op: 'open', create: false })
  socket.write(JSON.stringify({ version: 1, id: storage.id, operation: 'storage_reply', input: {
    version: 1, serial: storage.data.serial, index: 0, done: true,
    result: { ok: true, data: { absent: true } },
  } }) + '\n')
  assert.equal((await nextTerminal('failed')).id, 'inspect-records')
  await assert.rejects(fs.access(selected))
  socket.write(JSON.stringify({
    version: 1, id: 'templates', operation: 'options', input: { field: 'bundledTemplate' },
  }) + '\n')
  await nextTerminal('protocol_error')
  socket.end()
  const [code] = await closed
  assert.equal(code, 2, stderr)
})

test('real Electron profile paths use the prepared runtime grant and preserve workspace files', { timeout: 30000 }, async t => {
  const workspace = await fs.mkdtemp(path.join(build, 'profile-grant-'))
  const runtime = path.join(workspace, 'runtime')
  const configuration = path.join(workspace, 'configuration')
  for (const leaf of ['downloads', 'metadata', 'runtime', 'configuration']) await fs.mkdir(path.join(workspace, leaf))
  await fs.writeFile(path.join(configuration, 'authored'), 'preserved')
  const receipt = path.join(workspace, 'observed-profile.json')
  const bootstrap = path.join(workspace, 'observe-profile.cjs')
  await fs.writeFile(bootstrap, `// SPDX-License-Identifier: AGPL-3.0-or-later
const { app } = require('electron')
require(${JSON.stringify(path.join(build, 'worker/out/main/index.cjs'))})
require('node:fs').writeFileSync(${JSON.stringify(receipt)}, JSON.stringify({
  userData: app.getPath('userData'), sessionData: app.getPath('sessionData'), ready: app.isReady(),
}))
`)
  const server = createServer()
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const connected = once(server, 'connection', { signal: t.signal })
  const token = randomBytes(32).toString('hex')
  const env = { ...process.env }
  delete env.ELECTRON_RUN_AS_NODE
  delete env.NODE_OPTIONS
  const executable = process.platform === 'win32' ? 'electron.exe' :
    process.platform === 'darwin' ? 'Electron.app/Contents/MacOS/Electron' : 'electron'
  const child = spawn(path.join(build, 'electron', executable), [
    bootstrap, '--setup-workspace', workspace, '--setup-cache-runtime', runtime,
    '--setup-port', String(server.address().port), '--setup-token', token,
    '--setup-cache-downloads', path.join(workspace, 'downloads'), '--setup-cache-download-bytes', String(24 * 1024 ** 3),
    '--setup-cache-metadata', path.join(workspace, 'metadata'), '--setup-cache-metadata-bytes', String(256 * 1024 ** 2),
  ], { windowsHide: true, env, stdio: ['ignore', 'ignore', 'pipe'] })
  const closed = once(child, 'close')
  let stderr = '', socket, lines
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-8192) })
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill()
    socket?.destroy()
    server.close()
    lines?.close()
    await closed
  })
  ;[socket] = await Promise.race([connected, closed.then(([code, signal]) => {
    throw new Error(`Profile probe worker exited before connecting (${code}, ${signal}): ${stderr}`)
  })])
  server.close()
  lines = readline.createInterface({ input: socket, signal: t.signal })
  const frames = lines[Symbol.asyncIterator]()
  const authentication = await frames.next()
  assert.equal(JSON.parse(authentication.value).token, token)
  let ready = false
  for await (const line of frames) {
    const frame = JSON.parse(line)
    if (frame.event === 'metadata') {
      assert.ok(['read', 'abort'].includes(frame.data.operation.op))
      socket.write(JSON.stringify({ version: 1, id: frame.id, operation: 'metadata_reply', input: {
        version: 1, serial: frame.data.serial, index: 0, done: true,
        result: frame.data.operation.op === 'read' ? { ok: false, error: { code: 'ENOENT', osCode: 2 } } : { ok: true, data: {} },
      } }) + '\n')
    } else if (frame.event === 'ready') {
      ready = true
      break
    } else assert.equal(frame.event, 'diagnostic', JSON.stringify(frame))
  }
  assert.ok(ready, 'Actual worker readiness was not observed')
  const profiles = JSON.parse(await fs.readFile(receipt, 'utf8'))
  const normalize = value => process.platform === 'win32' ? path.resolve(value).toLowerCase() : path.resolve(value)
  assert.equal(normalize(profiles.userData), normalize(runtime))
  assert.equal(normalize(profiles.sessionData), normalize(runtime))
  assert.equal(profiles.ready, false, 'Profile selection must precede Electron readiness')
  socket.end()
  const [code] = await closed
  assert.equal(code, 0, stderr)
  assert.equal(await fs.readFile(path.join(configuration, 'authored'), 'utf8'), 'preserved')
  assert.deepEqual(await fs.readdir(configuration), ['authored'])
})
