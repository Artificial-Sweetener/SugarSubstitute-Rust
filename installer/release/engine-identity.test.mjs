// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Reject incomplete, ambiguous and lossy producing records before release assembly. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { engineBuildFile, readEngineIdentity } from './engine-identity.mjs'

async function fixture(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-engine-identity-'))
  t.after(async () => {
    assert.equal(path.dirname(await fs.realpath(root)), await fs.realpath(os.tmpdir()))
    await fs.rm(root, { recursive: true })
  })
  return { root, file: path.join(root, engineBuildFile) }
}

test('producing release preserves full u64 precision and does not change protocol', async t => {
  const { file } = await fixture(t)
  for (const release of ['1', '2', '9007199254740993', '18446744073709551615']) {
    const identity = { schema_version: 1, release, protocol: 1 }
    await fs.writeFile(file, JSON.stringify(identity) + '\n')
    assert.deepEqual(await readEngineIdentity(file), identity)
  }
})

test('unknown, duplicate, noncanonical and out-of-range declarations fail closed', async t => {
  const { file } = await fixture(t)
  const identity = { schema_version: 1, release: '2', protocol: 1 }
  const malformed = ['', 'null', '[]', ' '.repeat(513), '{"schema_version":1,"release":"1","release":"2","protocol":1}\n']
  for (const patch of [
    { schema_version: 2 }, { release: 2 }, { release: '0' }, { release: '02' },
    { release: '-1' }, { release: '18446744073709551616' }, { release: '1e3' },
    { protocol: 0 }, { protocol: 1.5 }, { protocol: 4294967296 }, { extra: true },
  ]) malformed.push(JSON.stringify({ ...identity, ...patch }) + '\n')
  for (const bytes of malformed) {
    await fs.writeFile(file, bytes)
    await assert.rejects(readEngineIdentity(file))
  }
  await assert.rejects(readEngineIdentity(undefined))
})

test('linked provenance cannot substitute a different producing directory', async t => {
  const { file, root } = await fixture(t)
  await fs.writeFile(file, '{"schema_version":1,"release":"2","protocol":1}\n')
  const linked = path.join(root, 'linked')
  await fs.symlink(root, linked, process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(readEngineIdentity(path.join(linked, engineBuildFile)), /links/)
})
