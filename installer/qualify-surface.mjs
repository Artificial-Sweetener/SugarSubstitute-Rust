// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Run offscreen rendering and behavior assertions without opening a desktop window. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
const root = path.dirname(fileURLToPath(import.meta.url))
const [source, output, scale = '1'] = process.argv.slice(2)
if (!['1', '1.25', '1.5', '2'].includes(scale)) throw new Error('Unsupported qualification display scale')
if (![source, output, process.env.SUBSTITUTE_INSTALLER_BUILD].every(value => value && path.isAbsolute(value))) {
  throw new Error('Provide absolute exported HTML, output and SUBSTITUTE_INSTALLER_BUILD paths')
}
await fs.mkdir(output)
const env = { ...process.env }
delete env.ELECTRON_RUN_AS_NODE
delete env.NODE_OPTIONS
const child = spawn(path.join(process.env.SUBSTITUTE_INSTALLER_BUILD, 'electron/electron.exe'),
  [path.join(root, 'tests/surface-runner.cjs'), source, output, scale], { windowsHide: true, env, stdio: 'pipe' })
let diagnostic = ''
child.stderr.on('data', bytes => { diagnostic = (diagnostic + bytes).slice(-8192) })
const code = await new Promise((resolve, reject) => {
  const timer = setTimeout(() => { child.kill(); reject(new Error('Offscreen qualification exceeded 60 seconds')) }, 60000)
  child.on('error', cause => { clearTimeout(timer); reject(cause) })
  child.on('close', code => { clearTimeout(timer); resolve(code) })
})
if (code !== 0) throw new Error(`Offscreen qualification failed: ${diagnostic}\n${await fs.readFile(path.join(output, 'surface-result.json'), 'utf8')}`)
console.log(await fs.readFile(path.join(output, 'surface-result.json'), 'utf8'))
