// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { build } from 'esbuild'
import { parseCommand } from '../worker/protocol.mjs'
import { recordHost } from './record-host.mjs'
const { outputFiles } = await build({ entryPoints: [fileURLToPath(new URL('../worker/records.ts', import.meta.url))], bundle: true, write: false, platform: 'node', format: 'esm' })
const { NativeRecords, nativeRecords } = await import('data:text/javascript;base64,' + Buffer.from(outputFiles[0].text).toString('base64'))
const artifact = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!artifact || !path.isAbsolute(artifact)) throw new Error('Explicit artifact directory required')
const selected = path.join(artifact, 'wire-records')

test('complete Unicode records retain the original 64 KiB bound through ordered chunks', async () => {
  const host = recordHost(NativeRecords, selected)
  await host.client.open(selected, true)
  const text = '糖😀'.repeat(9362) + 'ab'
  assert.equal(Buffer.byteLength(text), 65536)
  await host.client.publish(text)
  assert.equal((await host.client.read()).text, text)
  const chunks = host.operations.filter(operation => operation.op === 'chunk')
  assert.equal(chunks.length, 8)
  assert.ok(chunks.every(chunk => chunk.hex.length <= 16384))
  const before = host.operations.length
  await assert.rejects(host.client.publish(text + 'x'))
  assert.equal(host.operations.length, before)
})
test('foreign roots and absent native bindings cannot fall back to local files', async () => {
  assert.throws(() => nativeRecords())
  const host = recordHost(NativeRecords, selected)
  await assert.rejects(host.client.open(path.join(artifact, 'other'), true))
  await assert.rejects(host.client.open(path.parse(selected).root, true))
  assert.equal(host.operations.length, 0)
})
test('one actual pending exchange rejects overlap and retires on transport closure', async () => {
  const sent = [], client = new NativeRecords((id, data) => sent.push({ id, data }))
  client.begin('original', selected)
  const pending = client.open(selected, false)
  await assert.rejects(client.open(selected, false), /busy/)
  assert.equal(sent.length, 1)
  const rejected = assert.rejects(pending, /closed/)
  client.close()
  await rejected
  assert.throws(() => client.accept({ id: 'original', input: { serial: 1, index: 0, done: true, result: { ok: true, data: {} } } }))
})
test('foreign, skipped and duplicate reply identities fail before delivery', async () => {
  for (const input of [{ id: 'foreign', serial: 1, index: 0 }, { id: 'original', serial: 2, index: 0 }, { id: 'original', serial: 1, index: 1 }]) {
    const client = new NativeRecords(() => {})
    client.begin('original', selected)
    const pending = client.read()
    const rejected = assert.rejects(pending, /closed/)
    assert.throws(() => client.accept({ id: input.id, input: { ...input, done: true, result: { ok: true, data: {} } } }))
    client.close(); await rejected
  }
})
test('storage reply parsing rejects unknown authority, invalid statuses and excessive fragments', () => {
  const valid = { version: 1, serial: 1, index: 0, done: true, result: { ok: false, error: { code: 'ENOENT', osCode: 2 } } }
  const command = input => JSON.stringify({ version: 1, id: 'original', operation: 'storage_reply', input })
  assert.equal(parseCommand(command(valid)).input.result.error.osCode, 2)
  for (const input of [{ ...valid, path: selected }, { ...valid, index: 8 }, { ...valid, serial: 0 }, { ...valid, version: 2 },
    { ...valid, result: { ok: true, data: {}, error: {} } }, { ...valid, result: { ok: false, error: { code: 'UNKNOWN', osCode: null } } }]) {
    assert.throws(() => parseCommand(command(input)))
  }
})
test('native errors retain operating-system facts and request host-owned localized product copy', async () => {
  const client = new NativeRecords((id, data) => client.accept({ id, input: {
    serial: data.serial, index: 0, done: true, result: { ok: false, error: { code: 'EBUSY', osCode: 32 } },
  } }))
  client.begin('original', selected)
  await assert.rejects(client.read(), cause => cause.statusCode === 'setupFailed' && cause.code === 'EBUSY' && cause.osCode === 32)
})
test('read-only absence returns no managed-directory capability', async () => {
  const client = new NativeRecords((id, data) => client.accept({ id, input: {
    serial: data.serial, index: 0, done: true, result: { ok: true, data: { absent: true } },
  } }))
  client.begin('original', selected)
  assert.equal(await client.open(selected, false), undefined)
  assert.throws(() => client.child('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'))
})
