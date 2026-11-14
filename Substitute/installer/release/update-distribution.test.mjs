// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** A release build compiles its update distribution only for a version its changelog describes. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { changelogSection, releaseDistribution, repositoryRoot, writeUpdateDistribution } from './update-distribution.mjs'

/** A workspace at `version` with `changelog`, as semantic-release leaves it after a release. */
async function workspace(t, version, changelog) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-update-distribution-'))
  t.after(() => fs.rm(root, { recursive: true }))
  await fs.writeFile(path.join(root, 'Cargo.toml'), `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "${version}"\nedition = "2024"\n`)
  if (changelog !== undefined) await fs.writeFile(path.join(root, 'CHANGELOG.md'), changelog)
  return root
}

const released = [
  '## [0.2.0](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/compare/substitute-v0.1.0...substitute-v0.2.0) (2026-11-13)',
  '',
  '### Features',
  '',
  '* **updates:** show what changed ([abc1234](https://github.com/Artificial-Sweetener/SugarSubstitute-Rust/commit/abc1234))',
  '',
].join('\n')

test('the committed release distribution names its repository, feed, update policy and trusted root only', async () => {
  const distribution = JSON.parse(await fs.readFile(releaseDistribution, 'utf8'))
  assert.deepEqual(Object.keys(distribution).sort(), ['feed', 'repository', 'schema_version', 'trusted_root', 'update_policy'])
  assert.equal(distribution.schema_version, 4)
  assert.equal(distribution.trusted_root, await fs.readFile(repositoryRoot, 'utf8'))
  assert.equal(distribution.update_policy, 'beta_eager')
  assert.equal(distribution.feed.transport, 'https')
  assert.equal(distribution.repository.length, 32)
})

test('a released version gets the committed distribution, byte for byte', async t => {
  const root = await workspace(t, '0.2.0', `# Changelog\n\n${released}\n## [0.1.1](https://example.test) (2026-11-01)\n\n### Bug Fixes\n\n* older\n`)
  const output = path.join(root, 'update-distribution.json')
  const receipt = await writeUpdateDistribution({ workspace: root, output })
  assert.equal(receipt.version, '0.2.0')
  assert.deepEqual(await fs.readFile(output), await fs.readFile(releaseDistribution))
})

test('the section is exactly that version, up to the next one', () => {
  const changelog = `${released}\n## [0.1.1](https://example.test) (2026-11-01)\n\n* older\n`
  const section = changelogSection(changelog, '0.2.0')
  assert.match(section, /show what changed/)
  assert.doesNotMatch(section, /older/)
  assert.match(changelogSection('## 0.3.0 (2026-12-01)\n\n* first notes\n', '0.3.0'), /first notes/)
})

test('a version the changelog does not describe is never built with updates', async t => {
  for (const [version, changelog] of [
    ['0.1.0', released],
    ['0.2.1', released],
    ['10.2.0', released],
    ['0.2.0', released.replace('0.2.0](', '0.20.0](')],
    ['0.2.0', '## [0.2.0](https://example.test) (2026-11-13)\n'],
    ['0.2.0', ''],
  ]) {
    const root = await workspace(t, version, changelog)
    await assert.rejects(writeUpdateDistribution({ workspace: root, output: path.join(root, 'out.json') }), /CHANGELOG\.md/)
    await assert.rejects(fs.access(path.join(root, 'out.json')))
  }
  const missing = await workspace(t, '0.2.0', undefined)
  await assert.rejects(writeUpdateDistribution({ workspace: missing, output: path.join(missing, 'out.json') }), /ENOENT/)
})

test('an existing output, a relative output and a malformed source fail closed', async t => {
  const root = await workspace(t, '0.2.0', released)
  const output = path.join(root, 'update-distribution.json')
  await fs.writeFile(output, 'kept')
  await assert.rejects(writeUpdateDistribution({ workspace: root, output }), /EEXIST/)
  assert.equal(await fs.readFile(output, 'utf8'), 'kept')
  await assert.rejects(writeUpdateDistribution({ workspace: root, output: 'relative.json' }), /absolute/)
  const committed = JSON.parse(await fs.readFile(releaseDistribution, 'utf8'))
  for (const change of [{ schema_version: 3 }, { schema_version: 5 }, { trusted_root: '{}' }]) {
    const source = path.join(root, 'source.json')
    await fs.writeFile(source, JSON.stringify({ ...committed, ...change }))
    const fresh = path.join(root, `fresh-${Math.random().toString(16).slice(2)}.json`)
    await assert.rejects(writeUpdateDistribution({ workspace: root, source, output: fresh }), /schema 4|trusted root/i)
    await assert.rejects(fs.access(fresh))
  }
})
