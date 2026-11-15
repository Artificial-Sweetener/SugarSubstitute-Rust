// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { OutdatedPackError, requireSupportedPack } from '../worker/existing-pack.mjs'

/** A pack folder holding `files`, each with the given text. */
async function pack(t, files) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-existing-pack-'))
  t.after(() => fs.rm(root, { recursive: true, force: true }))
  for (const [name, text] of Object.entries(files)) {
    await fs.mkdir(path.dirname(path.join(root, name)), { recursive: true })
    await fs.writeFile(path.join(root, name), text)
  }
  return root
}

const current = {
  'pyproject.toml': '[project]\nname = "SugarCubes"\nversion = "0.16.0"\n',
  'load_model_roots.py': '',
  'sugarcubes/backend/self_update/release_source.json': '{}',
}

test('a SugarCubes that carries what this application works with is accepted', async t => {
  await requireSupportedPack(await pack(t, current))
})

test('SugarCubes 0.15.3, which predates the application, is refused by version with what to do', async t => {
  // The published 0.15.3 has neither the model-root owner nor a release-source description.
  const folder = await pack(t, { 'pyproject.toml': '[project]\nname = "SugarCubes"\nversion = "0.15.3"\n', '__init__.py': '' })
  await assert.rejects(requireSupportedPack(folder), error => {
    assert.ok(error instanceof OutdatedPackError)
    assert.equal(
      error.message,
      'SugarCubes 0.15.3 in this ComfyUI is older than SugarSubstitute works with. '
      + 'Update SugarCubes in that ComfyUI, or remove its SugarCubes folder from custom_nodes '
      + 'so setup installs the current version. Then run setup again.',
    )
    return true
  })
  assert.deepEqual((await fs.readdir(folder)).sort(), ['__init__.py', 'pyproject.toml'], 'the pack is left as it was')
})

test('a pack missing either part is refused, and one without readable metadata is still named', async t => {
  for (const missing of ['load_model_roots.py', 'sugarcubes/backend/self_update/release_source.json']) {
    const files = { ...current }
    delete files[missing]
    await assert.rejects(requireSupportedPack(await pack(t, files)), OutdatedPackError, missing)
  }
  await assert.rejects(requireSupportedPack(await pack(t, { '__init__.py': '' })), error => {
    assert.ok(error instanceof OutdatedPackError)
    assert.match(error.message, /^The SugarCubes in this ComfyUI is older/)
    return true
  })
})
