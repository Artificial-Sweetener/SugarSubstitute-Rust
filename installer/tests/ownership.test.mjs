// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { build as bundle } from 'esbuild'

const build = process.env.SUBSTITUTE_INSTALLER_BUILD
if (!build || !path.isAbsolute(build)) throw new Error('Explicit test artifact directory required')
async function module(name) {
  const { outputFiles } = await bundle({ entryPoints: [new URL(`../worker/${name}.ts`, import.meta.url).pathname.replace(/^\/([A-Z]:)/i, '$1')], bundle: true, write: false, platform: 'node', format: 'esm', target: 'node22' })
  return import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString('base64')}`)
}
const { reserve, record, ready } = await module('ownership')
const { downloadCache } = await module('cache')
const { CacheGrant } = await module('cache-policy')

test('failed and ready environments survive retries in the same managed destination', async () => {
  const workspace = await fs.mkdtemp(path.join(build, 'ownership-'))
  const target = path.join(workspace, 'ComfyUI')
  const first = await reserve(target)
  await fs.writeFile(path.join(first.directory, 'user-workflow.json'), 'preserve')
  await record(first, 'failed')
  const second = await reserve(target)
  assert.notEqual(first.directory, second.directory)
  await record(second, 'ready', { test: true })
  const third = await reserve(target)
  assert.notEqual(second.directory, third.directory)
  assert.equal(await fs.readFile(path.join(first.directory, 'user-workflow.json'), 'utf8'), 'preserve')
  assert.equal(JSON.parse(await fs.readFile(path.join(second.directory, '.substitute-provisioning.json'))).state, 'ready')
  await assert.rejects(record({ ...third, owner: first.owner }, 'failed'))
})

test('managed provisioning rejects unknown content and junction destinations', async () => {
  const workspace = await fs.mkdtemp(path.join(build, 'ownership-rejection-'))
  const unknown = path.join(workspace, 'existing')
  await fs.mkdir(unknown)
  await fs.writeFile(path.join(unknown, 'main.py'), 'preserve')
  await assert.rejects(reserve(unknown))
  assert.equal(await fs.readFile(path.join(unknown, 'main.py'), 'utf8'), 'preserve')
  const junction = path.join(workspace, 'junction')
  await fs.symlink(unknown, junction, process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(reserve(junction))
})

test('completed environments can be rediscovered and reused only for the same upstream plan', async () => {
  const workspace = await fs.mkdtemp(path.join(build, 'ownership-ready-'))
  const target = path.join(workspace, 'ComfyUI')
  assert.equal(await ready(target), undefined)
  const first = await reserve(target)
  const installation = (attempt, version, createdAt) => ({
    id: attempt.owner, installPath: attempt.directory, createdAt, version, variant: 'fixture',
  })
  await record(first, 'ready', installation(first, 'one', '2026-01-01T00:00:00Z'))
  const second = await reserve(target)
  await record(second, 'ready', installation(second, 'two', '2026-01-02T00:00:00Z'))
  assert.equal((await ready(target)).directory, second.directory)
  assert.equal((await ready(target, { version: 'one', variant: 'fixture' })).directory, first.directory)
  assert.equal(await ready(target, { version: 'three' }), undefined)
  const incomplete = await reserve(target)
  const marker = path.join(incomplete.directory, '.substitute-provisioning.json')
  await fs.writeFile(marker, '{')
  assert.equal((await ready(target)).directory, second.directory)
  assert.equal(await fs.readFile(marker, 'utf8'), '{')
  await record(second, 'failed', installation(second, 'two', '2026-01-02T00:00:00Z'))
  assert.equal((await ready(target)).directory, first.directory)
})

test('download cache reuses identities without evicting unknown files or failed partials', async () => {
  const workspace = await fs.mkdtemp(path.join(build, 'cache-'))
  const cache = downloadCache(new CacheGrant(workspace, 16 * 1024 * 1024))
  const entry = cache.getCachePath('../../upstream-key')
  assert.equal(path.dirname(entry), workspace)
  assert.equal(cache.getCachePath('../../upstream-key'), entry)
  await fs.writeFile(path.join(entry, 'archive.partial'), 'resume')
  await fs.mkdir(path.join(workspace, 'unknown'))
  await fs.writeFile(path.join(workspace, 'unknown', 'keep'), 'preserve')
  cache.evict()
  await cache.cleanPartials(0)
  assert.equal(await fs.readFile(path.join(entry, 'archive.partial'), 'utf8'), 'resume')
  assert.equal(await fs.readFile(path.join(workspace, 'unknown', 'keep'), 'utf8'), 'preserve')
  await fs.writeFile(path.join(entry, '.substitute-cache.json'), '{}')
  assert.throws(() => cache.getCachePath('../../upstream-key'))
})
