// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { writeInstallerDistribution } from './installer-distribution.mjs'

async function fixture(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-distribution-'))
  t.after(() => fs.rm(root, { recursive: true, force: true }))
  const role = path.join(root, 'role.json')
  await fs.writeFile(role, '\n{ "signed": {} }\r\n')
  return { root, role, output: path.join(root, 'distribution.json'), input: {
    repository: '92'.repeat(32), channel: 'release', manifestSha256: 'a'.repeat(64),
    trustedRoot: role, roots: [{ version: 2, file: role }], timestamp: role,
    snapshot: role, targets: role, descriptor: role,
  } }
}

test('distribution preserves exact role text and emits the native schema without modifying inputs', async t => {
  const { role, output, input } = await fixture(t)
  const before = await fs.readFile(role)
  const receipt = await writeInstallerDistribution(input, output)
  const bytes = await fs.readFile(output)
  const value = JSON.parse(bytes)
  assert.equal(value.schema_version, 1)
  assert.deepEqual(value.repository, [...Buffer.from(input.repository, 'hex')])
  assert.equal(value.channel, 'release')
  assert.equal(value.manifest_sha256, input.manifestSha256)
  for (const field of ['trusted_root', 'timestamp', 'snapshot', 'targets', 'descriptor']) assert.equal(value[field], before.toString('utf8'))
  assert.deepEqual(value.roots, [{ version: 2, metadata: before.toString('utf8') }])
  assert.equal(receipt.sha256, createHash('sha256').update(bytes).digest('hex'))
  assert.equal(receipt.bytes, bytes.length)
  assert.deepEqual(await fs.readFile(role), before)
})

test('occupied output is preserved and no replacement configuration is reported', async t => {
  const { output, input } = await fixture(t)
  await fs.writeFile(output, 'existing authorized distribution')
  await assert.rejects(writeInstallerDistribution(input, output), { code: 'EEXIST' })
  assert.equal(await fs.readFile(output, 'utf8'), 'existing authorized distribution')
})

test('invalid identities, root order, unknown fields and relative paths reject before publication', async t => {
  const { root, output, input } = await fixture(t)
  for (const changed of [
    { repository: '0'.repeat(64) }, { channel: 'nightly' }, { manifestSha256: 'A'.repeat(64) },
    { roots: [{ version: 2, file: input.trustedRoot }, { version: 2, file: input.trustedRoot }] },
    { roots: [{ version: 0, file: input.trustedRoot }] }, { trustedRoot: 'relative.json' },
    { unexpected: true }, { roots: [{ version: 2, file: input.trustedRoot, extra: true }] },
    { trustedRoot: root },
  ]) await assert.rejects(writeInstallerDistribution({ ...input, ...changed }, output))
  await assert.rejects(fs.stat(output), { code: 'ENOENT' })
})

test('oversized and malformed UTF-8 role bytes do not produce an installer configuration', async t => {
  const { role, output, input } = await fixture(t)
  await fs.writeFile(role, Buffer.alloc(1024 ** 2 + 1, 32))
  await assert.rejects(writeInstallerDistribution(input, output))
  await fs.writeFile(role, Buffer.from([0xc3, 0x28]))
  await assert.rejects(writeInstallerDistribution(input, output))
  await assert.rejects(fs.stat(output), { code: 'ENOENT' })
})

test('aggregate and escaped role bounds reject before reading later roots', async t => {
  const { root, role, output, input } = await fixture(t)
  const large = path.join(root, 'large.json')
  await fs.writeFile(large, ' '.repeat(1024 ** 2))
  const roots = [1, 2, 3, 4].map(version => ({ version, file: large }))
  roots.push({ version: 5, file: path.join(root, 'must-not-be-read.json') })
  await assert.rejects(writeInstallerDistribution({ ...input, roots }, output), /bound/)
  await fs.writeFile(role, '\u0000'.repeat(1024 ** 2))
  await assert.rejects(writeInstallerDistribution(input, output), /encoded bound/)
  await assert.rejects(fs.stat(output), { code: 'ENOENT' })
})
