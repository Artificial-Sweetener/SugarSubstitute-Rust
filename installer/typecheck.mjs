// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Check the adapter against actual patched upstream types and its explicit boundary replacements. */
import ts from 'typescript'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const root = path.dirname(fileURLToPath(import.meta.url))
const destination = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!destination || !path.isAbsolute(destination)) throw new Error('Explicit build directory required')
const receipt = JSON.parse(fs.readFileSync(path.join(destination, 'worker/build-receipt.json'), 'utf8'))
const input = receipt.inputs.find(value => value.includes('/src/main/sources/standalone/index.ts'))
if (!input) throw new Error('Actual upstream provisioning input missing from build receipt')
const upstream = input.slice(0, input.indexOf('/src/main/'))
const replacements = new Map([
  ['src/main/settings.ts', 'settings.ts'], ['src/main/lib/paths.ts', 'paths.ts'],
  ['src/main/lib/telemetry.ts', 'telemetry.ts'], ['src/main/lib/bundledScript.ts', 'bundled-script.ts'],
  ['src/main/lib/cache-policy.ts', 'cache-policy.ts'],
  ['src/main/lib/native-metadata.ts', 'metadata.ts'],
])
const options = {
  noEmit: true, strict: true, skipLibCheck: true, allowJs: true, checkJs: false,
  module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
  target: ts.ScriptTarget.ES2022, esModuleInterop: true,
  types: ['node'], typeRoots: [path.join(root, 'node_modules/@types')],
}
const host = ts.createCompilerHost(options)
host.resolveModuleNames = (names, containing) => names.map(name => {
  if (name.startsWith('@upstream/')) return { resolvedFileName: path.join(upstream, 'src/main', name.slice(10) + '.ts'), extension: ts.Extension.Ts }
  if (name.startsWith('.') && path.resolve(containing).startsWith(path.resolve(upstream) + path.sep)) {
    const relative = path.relative(upstream, path.resolve(path.dirname(containing), name) + '.ts').replaceAll('\\', '/')
    if (replacements.has(relative)) return { resolvedFileName: path.join(root, 'worker', replacements.get(relative)), extension: ts.Extension.Ts }
  }
  return ts.resolveModuleName(name, name.startsWith('.') ? containing : path.join(root, 'worker/main.ts'), options, host).resolvedModule
})
const program = ts.createProgram([path.join(root, 'worker/main.ts')], options, host)
const diagnostics = ts.getPreEmitDiagnostics(program)
if (diagnostics.length) {
  console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: name => name, getCurrentDirectory: () => root, getNewLine: () => '\n',
  }))
  process.exitCode = 1
} else console.log('Provisioning adapter and consumed upstream types passed strict TypeScript checks')
