// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { Events } from '../worker/events.mjs'

test('bursty copy progress stays bounded and completion follows the latest progress', () => {
  const received = []
  const events = new Events((id, event, data) => received.push({ id, event, data }))
  for (let count = 0; count < 100000; count++) {
    events.send('install', 'progress', { percent: count / 1000 })
    events.send('install', 'diagnostic', { message: `file ${count}` })
  }
  assert.equal(received.length, 0)
  events.send('install', 'completed', { ready: true })
  assert.deepEqual(received.map(item => item.event), ['diagnostic', 'progress', 'completed'])
  assert.equal(received[0].data.omitted, 99992)
  assert.ok(received[0].data.message.endsWith('file 99999'))
  assert.equal(received[1].data.percent, 99.999)
  events.flush()
  assert.equal(received.length, 3)
})
