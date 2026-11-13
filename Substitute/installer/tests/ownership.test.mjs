// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { build as bundle } from 'esbuild'
import { recordHost } from './record-host.mjs'

const build = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!build || !path.isAbsolute(build)) throw new Error('Explicit test artifact directory required')
async function module(name) {
  const { outputFiles } = await bundle({ stdin: { contents: `export * from './${name}'; ${name === 'ownership' ? "export * from './records'; export { bindDiagnostics } from './diagnostics.mjs'" : name === 'cache' ? "export * from './downloads'" : ''}`, resolveDir: new URL('../worker', import.meta.url).pathname.replace(/^\/([A-Z]:)/i, '$1'), loader: 'ts' }, bundle: true, write: false, platform: 'node', format: 'esm', target: 'node22' })
  return import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString('base64')}`)
}
const { reserve, record, ready, NativeRecords, bindNativeRecords, bindDiagnostics } = await module('ownership')
const diagnostics = []
bindDiagnostics((level, message) => diagnostics.push({ level, message }))
const { downloadCache, NativeDownloads, bindNativeDownloads } = await module('cache')

function fixture(name) {
  const target = path.join(build, name, 'ComfyUI')
  const host = recordHost(NativeRecords, target)
  bindNativeRecords(host.client)
  return { ...host, target }
}

test('failed and ready environments survive retries through the native record port', async () => {
  const host = fixture('schema-retries')
  const first = await reserve(host.target)
  host.files.set(path.join(first.directory, 'user-workflow.json'), 'preserve')
  await record(first, 'failed')
  const second = await reserve(host.target)
  await record(second, 'ready', { test: true })
  const third = await reserve(host.target)
  assert.notEqual(first.directory, second.directory)
  assert.notEqual(second.directory, third.directory)
  assert.equal(host.files.get(path.join(first.directory, 'user-workflow.json')), 'preserve')
  assert.equal(JSON.parse(host.files.get(path.join(second.directory, '.substitute-provisioning.json'))).state, 'ready')
  await assert.rejects(record({ ...third, owner: first.owner }, 'failed'))
  assert.ok(host.operations.some(operation => operation.op === 'begin' && operation.revision > 0))
})

test('managed schemas preserve unknown content and foreign ownership', async () => {
  const host = fixture('schema-rejection')
  host.directories.add(host.target)
  const authored = path.join(host.target, 'main.py')
  host.files.set(authored, 'preserve')
  await assert.rejects(reserve(host.target))
  assert.equal(host.files.get(authored), 'preserve')
  assert.equal(host.operations.some(operation => operation.op === 'begin'), false)
  host.files.clear()
  const marker = path.join(host.target, '.substitute-managed.json')
  host.files.set(marker, '{"schema_version":2,"product":"SugarSubstitute","kind":"managed-comfy"}')
  await assert.rejects(reserve(host.target))
  assert.equal(JSON.parse(host.files.get(marker)).schema_version, 2)
})

test('completed schemas reuse only the matching plan and preserve unreadable attempts', async () => {
  const host = fixture('schema-ready')
  assert.equal(await ready(host.target), undefined)
  const first = await reserve(host.target)
  const installation = (attempt, version, createdAt) => ({ id: attempt.owner, installPath: attempt.directory, createdAt, version, variant: 'fixture' })
  await record(first, 'ready', installation(first, 'one', '2026-01-01T00:00:00Z'))
  const second = await reserve(host.target)
  await record(second, 'ready', installation(second, 'two', '2026-01-02T00:00:00Z'))
  assert.equal((await ready(host.target)).directory, second.directory)
  assert.equal((await ready(host.target, { version: 'one', variant: 'fixture' })).directory, first.directory)
  assert.equal(await ready(host.target, { version: 'three' }), undefined)
  const incomplete = await reserve(host.target)
  const marker = path.join(incomplete.directory, '.substitute-provisioning.json')
  host.files.set(marker, '{')
  assert.equal((await ready(host.target)).directory, second.directory)
  assert.equal(host.files.get(marker), '{')
  assert.ok(diagnostics.some(event => event.level === 'warn' && event.message.includes('Preserving unreadable provisioning attempt')))
  await record(second, 'failed', installation(second, 'two', '2026-01-02T00:00:00Z'))
  assert.equal((await ready(host.target)).directory, first.directory)
})

test('download cache delegates identities and facts to the scoped native owner', async () => {
  const workspace = await fs.mkdtemp(path.join(build, 'cache-'))
  const entry = path.join(workspace, 'a'.repeat(64)), operations = []
  const client = new NativeDownloads((id, data) => {
    operations.push(data.operation)
    const result = data.operation.op === 'entry' ? { path: entry } :
      data.operation.op === 'length' ? { length: 16 } : {}
    queueMicrotask(() => client.accept({ id, input: { serial: data.serial, index: 0, done: true, result: { ok: true, data: result } } }))
  }, workspace)
  client.begin('cache-test')
  bindNativeDownloads(client)
  const cache = downloadCache()
  assert.equal(await cache.getCachePath('../../upstream-key'), entry)
  assert.equal(await cache.getCachePath('../../upstream-key'), entry)
  assert.equal(await cache.length(path.join(entry, 'payload')), 16)
  await cache.touch('../../upstream-key')
  await cache.evict()
  await cache.cleanPartials(0)
  assert.deepEqual(operations.map(operation => operation.op), ['entry', 'entry', 'length', 'entry', 'validate', 'validate'])
  assert.equal(operations[0].key, '../../upstream-key')
  assert.equal((await fs.readdir(workspace)).length, 0)
})
