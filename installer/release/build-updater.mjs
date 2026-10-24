// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Build and seal an independent native updater; signing and publication are separate release steps. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { buildNative } from './native-build.mjs'
import { assembleArtifacts } from './application-artifacts.mjs'
import { packageUpdater } from './updater-package.mjs'
import { windowsInspector } from './windows-imports.mjs'
import { newArtifactDirectory } from './artifact-files.mjs'
import { engineBuildFile } from './engine-identity.mjs'

const [output, profile, platform, release, reader, ...extra] = process.argv.slice(2)
const qualification = ['--local-k1', '--local-updates'].includes(extra.at(-1)) ? extra.pop() : undefined
if (process.platform !== 'win32' || !output || !path.isAbsolute(output) || !['dev', 'release'].includes(profile) || !release || !reader) {
  throw new Error('Usage on Windows: node installer/release/build-updater.mjs <new-absolute-output> <dev|release> <windows-x86_64|windows-aarch64> <release-id> <llvm-readobj> [absolute-runtime-dll ...] [--local-k1|--local-updates]')
}
const inspect = await windowsInspector(reader, platform)
await newArtifactDirectory(output)
const selection = await buildNative('updater', profile, qualification, platform, inspect)
const build = await assembleArtifacts(selection, path.join(output, 'build'))
const bundle = path.join(output, 'package')
const copied = { ...selection, engineBuild: path.join(output, 'build', engineBuildFile), files: new Map([...selection.files.keys()].map(name => [name, path.join(output, 'build', name)])) }
const payload = await packageUpdater(copied, extra, bundle, release, inspect)
const archive = path.join(output, 'updater.ssupdate')
const sealed = spawnSync(process.execPath, [fileURLToPath(new URL('../seal.mjs', import.meta.url)), '--update', bundle, archive], { encoding: 'utf8', windowsHide: true, timeout: 15 * 60 * 1000, maxBuffer: 1024 ** 2 })
if (sealed.error) throw sealed.error
if (sealed.status !== 0) throw new Error(`Updater sealing failed: ${sealed.stderr}`)
const receipt = { schema_version: 1, platform, release, build, ...payload, archive: JSON.parse(sealed.stdout) }
await fs.writeFile(path.join(output, 'updater-package-build.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' })
console.log(JSON.stringify(receipt))
