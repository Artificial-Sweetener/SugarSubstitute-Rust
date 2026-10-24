// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Read the real upstream catalogue and optionally inspect an existing Comfy installation. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { createServer } from 'node:net'
import { randomBytes } from 'node:crypto'
import readline from 'node:readline'
const build = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!build || !path.isAbsolute(build)) throw new Error('Explicit worker build required')
const workspace = await fs.mkdtemp(path.join(build, 'catalogue-'))
const server = createServer()
server.listen(0, '127.0.0.1')
await once(server, 'listening')
const connected = once(server, 'connection')
const token = randomBytes(32).toString('hex')
const env = { ...process.env }
delete env.ELECTRON_RUN_AS_NODE
delete env.NODE_OPTIONS
const child = spawn(path.join(build, 'electron/electron.exe'), [
  path.join(build, 'worker'), '--setup-port', String(server.address().port),
  '--setup-token', token, '--setup-workspace', workspace,
], { windowsHide: true, env, stdio: 'pipe' })
let stderr = ''
child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192) })
const closed = once(child, 'close')
let socket
let lines
const timer = setTimeout(() => { child.kill(); socket?.destroy(); server.close() }, 120000)
try {
  [socket] = await Promise.race([connected, closed.then(() => { throw new Error(`Worker closed before connecting: ${stderr}`) })])
  server.close()
  lines = readline.createInterface({ input: socket })
  const iterator = lines[Symbol.asyncIterator]()
  const next = async () => {
    const item = await iterator.next()
    if (item.done) throw new Error(`Worker disconnected: ${stderr}`)
    return JSON.parse(item.value)
  }
  if ((await next()).token !== token) throw new Error('Wrong worker authentication')
  let first = await next()
  while (first.event === 'diagnostic') first = await next()
  if (first.event !== 'ready') throw new Error(`Worker not ready: ${JSON.stringify(first)}`)
  let sequence = 0
  async function request(operation, input) {
    const id = String(++sequence)
    socket.write(JSON.stringify({ version: 1, id, operation, input }) + '\n')
    for (;;) {
      const event = await next()
      if (event.event === 'diagnostic') continue
      if (event.id !== id || event.event !== 'completed') throw new Error(JSON.stringify(event))
      return event.data
    }
  }
  const selections = {}
  const catalogue = {}
  for (const field of ['release', 'comfyVersion', 'variant']) {
    const values = await request('options', { field, selections, locale: 'en', mirrors: false })
    catalogue[field] = values
    const chosen = values.find(value => value.recommended) ?? values[0]
    if (chosen) selections[field] = chosen.value
  }
  const environment = process.argv[2] ? await request('inspect', { directory: process.argv[2] }) : null
  await fs.writeFile(path.join(workspace, 'qualification.json'), JSON.stringify({ catalogue, environment }, null, 2))
  socket.end()
  const [code] = await closed
  if (code !== 0) throw new Error(`Worker failed to close: ${code}`)
  console.log(JSON.stringify({ passed: true, options: Object.fromEntries(Object.entries(catalogue).map(([key, values]) => [key, values.length])), environment, evidence: workspace }))
} finally {
  clearTimeout(timer)
  socket?.destroy()
  lines?.close()
  server.close()
  if (child.exitCode === null) child.kill()
  await closed
}
