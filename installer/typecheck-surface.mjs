// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Strictly check the authored renderer against the actual Rust-owned serialized message keys. */
import ts from 'typescript'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const root = path.dirname(fileURLToPath(import.meta.url))
const source = fs.readFileSync(path.join(root, '../crates/sugarsubstitute-contracts/src/localization/installer.rs'), 'utf8')
const fields = [...source.matchAll(/(?:#\[serde\(rename = "([^"]+)"\)\]\s*)?pub (\w+): &'static str,/g)]
if (!source.includes('#[serde(rename_all = "camelCase")]') || !fields.length) throw new Error('Review installer message serialization before typechecking')
const catalog = fields.map(([, rename, field]) => `${JSON.stringify(rename ?? field.replace(/_([a-z])/g, (_, c) => c.toUpperCase()))}: string;`).join('\n')
const virtual = path.join(root, 'surface/messages.d.ts')
const options = {
  strict: true, allowJs: true, checkJs: true, noEmit: true, skipLibCheck: true,
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler, types: [],
}
const host = ts.createCompilerHost(options)
const read = host.readFile
const exists = host.fileExists
host.readFile = file => path.resolve(file) === virtual ? `export interface InstallerMessages {${catalog}}` : read(file)
host.fileExists = file => path.resolve(file) === virtual || exists(file)
const program = ts.createProgram([path.join(root, 'surface/controller.js'), path.join(root, 'surface/contracts.d.ts')], options, host)
const diagnostics = ts.getPreEmitDiagnostics(program)
if (diagnostics.length) {
  console.error(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCanonicalFileName: name => name, getCurrentDirectory: () => root, getNewLine: () => '\n',
  }))
  process.exitCode = 1
} else console.log(`Installer renderer passed strict JavaScript checks against ${fields.length} Rust-owned message keys`)
