// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { Decoder, parseCommand, MAX_FRAME_BYTES } from '../worker/protocol.mjs'

const command = (operation, input = {}) => JSON.stringify({ version: 1, id: 'request-1', operation, input })

test('proxy replies preserve ordered routes and reject credentials, authority injection and oversized lists', () => {
  const routes = [{ kind: 'proxy', url: 'http://proxy.test:8080/' }, { kind: 'direct' }]
  assert.deepEqual(parseCommand(command('proxy_result', { routes })).input.routes, routes)
  for (const input of [{}, { routes: [] }, { routes, error: 'ambiguous' }, { routes: Array(17).fill({ kind: 'direct' }) },
    ...['socks://proxy.test/', 'http://user:secret@proxy.test/', 'http://proxy.test/path', 'https://proxy.test/?secret'].map(url => ({ routes: [{ kind: 'proxy', url }] })),
    { routes: [{ kind: 'direct', url: 'http://injected.test/' }] }, { routes: [null] }, { error: 'x'.repeat(2049) }]) {
    assert.throws(() => parseCommand(command('proxy_result', input)))
  }
})

test('fragmented UTF-8 frames preserve exact selected paths', () => {
  const wire = Buffer.from(command('inspect', { directory: 'E:\\作品\\ComfyUI' }) + '\n')
  const decoder = new Decoder()
  const frames = []
  for (const byte of wire) frames.push(...decoder.push(Buffer.from([byte])))
  decoder.end()
  assert.equal(frames.length, 1)
  assert.equal(frames[0].input.directory, 'E:\\作品\\ComfyUI')
})

test('unknown operations, fields and download authority are rejected', () => {
  for (const value of [
    command('shell', { command: 'calc' }),
    command('provision', { directory: 'E:\\ComfyUI', selectionToken: 'id', downloadUrl: 'https://example.test' }),
    command('options', { field: 'bundledTemplate' }),
    command('options', { field: 'variant', selections: { variant: { data: {} } } }),
    command('options', { field: 'release', mirrors: 'yes' }),
    JSON.stringify({ version: 2, id: 'x', operation: 'inspect', input: { directory: 'x' } }),
    command('inspect', { directory: 'x\0y' }),
    command('integrate', { directory: 'x', python: 'python', frontend: 'frontend', metadata: 'metadata', packages: ['untrusted'] }),
    command('integrate', { directory: 'x' }),
  ]) assert.throws(() => parseCommand(value))
})

test('integration accepts only the verified runtime paths without package or command authority', () => {
  const runtime = { directory: 'E:\\ComfyUI', python: 'E:\\Python\\python.exe', frontend: 'E:\\frontend', metadata: 'E:\\METADATA', models: 'E:\\Models' }
  assert.deepEqual(parseCommand(command('integrate', runtime)).input, runtime)
})

test('unterminated and oversized frames fail without unbounded buffering', () => {
  const decoder = new Decoder()
  decoder.push(Buffer.alloc(MAX_FRAME_BYTES, 32))
  assert.throws(() => decoder.push(Buffer.from('x')))
  const truncated = new Decoder()
  truncated.push(Buffer.from('{"version":1'))
  assert.throws(() => truncated.end())
})

test('multiple complete frames are delivered in order', () => {
  const decoder = new Decoder()
  const frames = decoder.push(Buffer.from(command('options', { field: 'release' }) + '\n' +
    command('cancel', { requestId: 'request-1' }) + '\n'))
  assert.deepEqual(frames.map(item => item.operation), ['options', 'cancel'])
})
