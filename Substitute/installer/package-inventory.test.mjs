// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Producer-side package bounds preserve Unicode names and reject native path aliases. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { writeInventory } from './package-inventory.mjs'

test('shared inventory accepts Unicode and spaces and rejects aliases and overflow before publication', async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-inventory-'))
  t.after(async () => {
    assert.equal(path.dirname(await fs.realpath(root)), await fs.realpath(os.tmpdir()))
    await fs.rm(root, { recursive: true })
  })
  const entry = { path: 'app/文書 notes.txt', bytes: 1, sha256: '1'.repeat(64) }
  for (const name of ['app/../outside', 'app/con.txt', 'app/LPT².txt', 'app/name.', 'app/name ', 'app/stream:payload', 'app/back\\slash', 'app/control\u0001', 'app/a'.repeat(400), 'other/file']) {
    await assert.rejects(writeInventory(root, 'engine-1', [{ ...entry, path: name }]))
    await assert.rejects(fs.stat(path.join(root, 'setup-manifest.json')), { code: 'ENOENT' })
  }
  await assert.rejects(writeInventory(root, 'engine-1', [entry, entry]))
  await assert.rejects(writeInventory(root, 'engine-1', [{ ...entry, bytes: 2 * 1024 ** 3 + 1 }]))
  await writeInventory(root, 'engine-1', [entry])
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(root, 'setup-manifest.json'), 'utf8')).files, [entry])
})
