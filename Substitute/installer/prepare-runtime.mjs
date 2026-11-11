// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Prepare the checksum-pinned standalone runtime in a new explicit artifact directory. */
import { createRequire } from 'node:module'
import fs from 'node:fs/promises'
import { createReadStream, createWriteStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { Readable, Transform } from 'node:stream'
import { pipeline } from 'node:stream/promises'
import { execFileSync } from 'node:child_process'
import path from 'node:path'
import { runtime, runtimeArchive, runtimeExecutable } from './runtime.mjs'
const require = createRequire(import.meta.url)
const destination = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!destination || !path.isAbsolute(destination)) throw new Error('An absolute build directory is required')
const pin = runtimeArchive()
await fs.mkdir(destination, { recursive: true })
const staging = await fs.mkdtemp(path.join(destination, 'node-runtime-'))
const archive = path.join(staging, pin.file)
const response = await fetch(pin.url, { signal: AbortSignal.timeout(300_000) })
if (!response.ok || !response.body) throw new Error(`Runtime download failed: HTTP ${response.status}`)
let received = 0
await pipeline(Readable.fromWeb(response.body), new Transform({
  transform(chunk, _, done) {
    received += chunk.length
    done(received > 128 * 1024 * 1024 ? new Error('Runtime archive exceeds its bound') : null, chunk)
  },
}), createWriteStream(archive, { flags: 'wx' }))
const digest = createHash('sha256')
for await (const bytes of createReadStream(archive)) digest.update(bytes)
if (digest.digest('hex') !== pin.sha256) throw new Error('Runtime archive authentication failed')
const extracted = path.join(staging, 'extracted')
await fs.mkdir(extracted)
execFileSync('tar', ['-xf', archive, '-C', extracted], { windowsHide: true })
const folder = pin.file.replace(/\.(zip|tar\.(gz|xz))$/, '')
const source = path.join(extracted, folder)
const target = path.dirname(runtimeExecutable(destination))
await fs.mkdir(path.dirname(target), { recursive: true })
await fs.mkdir(target)
await fs.copyFile(path.join(source, process.platform === 'win32' ? 'node.exe' : 'bin/node'), runtimeExecutable(destination), fs.constants.COPYFILE_EXCL)
await fs.copyFile(path.join(source, 'LICENSE'), path.join(target, 'LICENSE'), fs.constants.COPYFILE_EXCL)
const version = execFileSync(runtimeExecutable(destination), ['--version'], { encoding: 'utf8', windowsHide: true }).trim()
if (version !== `v${runtime.version}`) throw new Error('Extracted runtime does not match its pin')
await fs.cp(path.dirname(require.resolve('7zip-bin/package.json')),
  path.join(destination, 'worker/node_modules/7zip-bin'), { recursive: true, errorOnExist: true, force: false })
console.log(`Prepared Node.js ${runtime.version} at ${target}`)
