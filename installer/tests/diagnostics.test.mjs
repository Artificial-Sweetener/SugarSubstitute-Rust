// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { diagnosticText, diagnosticMessage, bindDiagnostics, diagnostic } from '../worker/diagnostics.mjs'
import { Events } from '../worker/events.mjs'

test('UTF-8 byte admission covers individual and combined Unicode messages', () => {
  assert.equal(diagnosticText('糖'.repeat(1366)), '[oversized diagnostic]')
  assert.ok(Buffer.byteLength(diagnosticMessage(['糖'.repeat(1000), '糖'.repeat(1000)])) <= 4096)
  assert.ok(Buffer.byteLength(diagnosticMessage(['x'.repeat(4096), 'later'])) <= 4096)
})
test('routing preserves the identical terminal sink failure', () => {
  const cause = new Error('sink failure')
  bindDiagnostics(() => { throw cause })
  assert.throws(() => diagnostic('warn', 'bounded observation'), error => error === cause)
  assert.throws(() => bindDiagnostics(() => {}), /already bound/)
})

test('different requests keep their actual diagnostic correlation during coalescing', () => {
  const received = []
  const events = new Events((id, event, data) => received.push({ id, event, data }))
  events.send('first', 'diagnostic', { message: 'first request' })
  events.send('second', 'diagnostic', { message: 'second request' })
  events.flush()
  assert.deepEqual(received.map(value => [value.id, value.data.message]), [['first', 'first request'], ['second', 'second request']])
})

test('untrusted diagnostics redact secrets and paths before queue retention', () => {
  for (const value of ['Bearer secret', 'Bearer\tabc', 'Bearer\u0085abc', 'token=secret', 'C:\\Users\\private', 'https://host/path?key=secret']) {
    assert.equal(diagnosticText(value), '[redacted]')
  }
  assert.equal(diagnosticText('a\nb\0c'), 'a b c')
  assert.equal(diagnosticText('x'.repeat(4097)), '[oversized diagnostic]')
})

test('object diagnostics never execute serialization or custom inspection', () => {
  const hostile = { toString() { throw new Error('must not inspect') }, toJSON() { throw new Error('must not serialize') } }
  assert.equal(diagnosticMessage([hostile]), '[untrusted diagnostic]')
  const received = []
  const events = new Events((id, event, data) => received.push({ id, event, data }))
  events.send('request', 'diagnostic', { message: hostile, properties: hostile })
  events.flush()
  assert.equal(received[0].data.message, '[untrusted diagnostic]')
})

test('retained diagnostic copies cannot change after enqueue and stay bounded under overload', () => {
  const received = []
  const events = new Events((id, event, data) => received.push({ id, event, data }))
  const data = { message: 'original' }
  events.send('first', 'diagnostic', data)
  data.message = 'token=secret'
  events.flush()
  assert.equal(received[0].data.message, 'original')
  for (let i = 0; i < 100; i++) events.send('request', 'diagnostic', { message: 'x'.repeat(4096) })
  events.send('request', 'completed', { result: true })
  assert.equal(received[1].data.omitted, 92)
  assert.ok(Buffer.byteLength(received[1].data.message) <= 8 * 4096 + 7)
  assert.equal(received[2].event, 'completed')
})
