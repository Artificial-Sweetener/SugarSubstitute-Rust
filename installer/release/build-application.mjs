// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Build explicit native targets and collect their compiler-produced launch declaration. */
import path from 'node:path'
import { assembleArtifacts } from './application-artifacts.mjs'
import { buildNative } from './native-build.mjs'
import { windowsInspector } from './windows-imports.mjs'

const [output, profile, platform, reader, qualification, ...extra] = process.argv.slice(2)
if (process.platform !== 'win32' || !output || !path.isAbsolute(output) || !reader || !['dev', 'release'].includes(profile) || extra.length) {
  throw new Error('Usage on Windows: node installer/release/build-application.mjs <new-absolute-output> <dev|release> <windows-x86_64|windows-aarch64> <llvm-readobj> [--local-k1|--local-updates]')
}
const inspect = await windowsInspector(reader, platform)
const receipt = await assembleArtifacts(await buildNative('application', profile, qualification, platform, inspect), output)
console.log(JSON.stringify({ output, ...receipt }))
