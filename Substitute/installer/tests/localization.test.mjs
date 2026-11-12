// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

/** Check real imports and source types without creating a second catalog or disk fixture. */
function diagnostics(statement) {
  const fixture = path.join(root, 'worker/localization-rejection.ts')
  const options = { noEmit: true, strict: true, skipLibCheck: true, types: [], target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler }
  const host = ts.createCompilerHost(options)
  const read = host.getSourceFile
  const exists = host.fileExists
  host.fileExists = file => path.resolve(file) === fixture || exists(file)
  host.getSourceFile = (file, language, ...rest) => path.resolve(file) === fixture ? ts.createSourceFile(file,
    `import {ProductFailure, type ProgressSink} from './product-messages'; declare const send: ProgressSink; ${statement}`,
    language, true) : read(file, language, ...rest)
  return ts.getPreEmitDiagnostics(ts.createProgram([fixture], options, host))
}

test('owned worker progress cannot publish literals or invent product codes', () => {
  for (const statement of [
    "send('progress', {phase:'verify',percent:0,status:'Checking the Python environment'});",
    "send('progress', {phase:'verify',percent:0,statusCode:'invented'});",
    "send('progress', {phase:'verify',percent:0,statusCode:'setupFailed'});",
    "new ProductFailure('invented','Original cause');",
  ]) assert.ok(diagnostics(statement).length, statement)
  assert.deepEqual(diagnostics("send('progress',{phase:'verify',percent:0,statusCode:'checkingPython'}); new ProductFailure('selectPython','Original cause');"), [])
  assert.deepEqual(diagnostics("send('progress',{phase:'download',percent:0,status:'Upstream Unicode 日本語 😀',textOwner:'comfy-desktop'});"), [])
})

test('product failure preserves its original diagnostic separately from its host-resolved code', async () => {
  const source = await fs.readFile(path.join(root, 'worker/product-messages.ts'), 'utf8')
  const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText
  const { ProductFailure } = await import('data:text/javascript;base64,' + Buffer.from(compiled).toString('base64'))
  const failure = new ProductFailure('selectPython', 'Original cause 日本語 😀 e\u0301')
  assert.equal(failure.statusCode, 'selectPython')
  assert.equal(failure.message, 'Original cause 日本語 😀 e\u0301')
  assert.ok(failure instanceof Error)
})
