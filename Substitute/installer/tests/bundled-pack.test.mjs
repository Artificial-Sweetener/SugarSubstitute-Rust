// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { path7za } from '7zip-bin'
import { stageBundledPack } from '../worker/bundled-pack.mjs'

async function scratch(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-bundled-pack-'))
  t.after(() => fs.rm(root, { recursive: true, force: true }))
  return root
}

test('integration reads the pack setup will install from the bundled registry archive', async t => {
  const root = await scratch(t)
  const pack = path.join(root, 'pack')
  await fs.mkdir(path.join(pack, 'sugarcubes_model_roots'), { recursive: true })
  await fs.writeFile(path.join(pack, 'pyproject.toml'), '[project]\nname = "SugarCubes"\n')
  await fs.writeFile(path.join(pack, 'load_model_roots.py'), '# loader\n')
  await fs.writeFile(path.join(pack, 'sugarcubes_model_roots', '__init__.py'), '# api\n')
  const archive = path.join(root, 'SugarCubes.zip')
  await promisify(execFile)(path7za, ['a', '-tzip', '-bd', archive, '.\\*'], { cwd: pack, windowsHide: true })
  const state = path.join(root, 'state')
  const staged = await stageBundledPack(archive, state, path7za, new AbortController().signal)
  assert.ok(staged.folder.startsWith(state + path.sep))
  assert.equal(await fs.readFile(path.join(staged.folder, 'pyproject.toml'), 'utf8'), '[project]\nname = "SugarCubes"\n')
  assert.equal(await fs.readFile(path.join(staged.folder, 'sugarcubes_model_roots', '__init__.py'), 'utf8'), '# api\n')
  await staged.dispose()
  assert.deepEqual(await fs.readdir(state), [])
})

test('an unreadable bundled archive fails integration and leaves no staged folder', async t => {
  const root = await scratch(t)
  const state = path.join(root, 'state')
  const archive = path.join(root, 'SugarCubes.zip')
  await fs.writeFile(archive, 'not an archive')
  await assert.rejects(stageBundledPack(archive, state, path7za, new AbortController().signal))
  assert.deepEqual(await fs.readdir(state), [])
  await assert.rejects(stageBundledPack(path.join(root, 'absent.zip'), state, path7za, new AbortController().signal), { code: 'ENOENT' })
})