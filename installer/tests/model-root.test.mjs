// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { configureModelRoot } from '../worker/model-root.mjs'

// The external public API is faked here; its codecs and actual filesystem semantics have
// independent owner tests. No model-root JSON format is reproduced by this adapter fixture.
async function fixture(t, mode) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-model-integration-'))
  const extension = path.join(root, 'extension')
  t.after(async () => {
    for (const filename of [path.join(extension, 'trace.jsonl'), path.join(extension, 'load_model_roots.py'), path.join(root, 'untouched-control')]) {
      try { await fs.unlink(filename) } catch (cause) { if (cause.code !== 'ENOENT') throw cause }
    }
    await fs.rmdir(extension)
    await fs.rmdir(root)
  })
  const storage = path.join(root, 'user', 'comfy-data')
  const models = path.join(root, 'existing 模型 café')
  await fs.mkdir(extension)
  const trace = path.join(extension, 'trace.jsonl')
  const source = `
import json, pathlib, types
def trace(value):
    with (pathlib.Path(__file__).parent / 'trace.jsonl').open('a', encoding='utf-8') as output:
        output.write(json.dumps(value, ensure_ascii=False) + '\\n')
class Store:
    def __init__(self, root):
        self.root, self.selected = root, None
    @property
    def default_root(self):
        return self.root / 'models'
    ${mode === 'missing' ? 'pass' : `def read_preserved_selection(self):
        trace({'call': 'read', 'root': str(self.root)})
        ${mode === 'refused' ? "raise ValueError('selected control refused') from OSError('original owner cause')" : 'return None'}`}
    def load(self):
        return self.selected
class Configuration:
    def __init__(self, store, active):
        self.store = store
    def configure(self, mode, selected):
        trace({'call': 'configure', 'root': str(self.store.root), 'mode': mode, 'models': selected})
        self.store.selected = pathlib.Path(selected).resolve()
def load_model_root_api():
    return types.SimpleNamespace(MODEL_ROOT_API_VERSION=1, ModelRootStore=Store, ModelRootConfiguration=Configuration)
`
  await fs.writeFile(path.join(extension, 'load_model_roots.py'), source)
  const invoke = () => {
    const operation = promisify(execFile)(process.env.SUBSTITUTE_TEST_PYTHON ?? 'python',
      ['-B', '-c', configureModelRoot, extension, storage, models], {
        windowsHide: true, env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' },
      })
    t.diagnostic(`model owner Python pid=${operation.child.pid}`)
    return operation
  }
  const calls = async () => {
    try { return (await fs.readFile(trace, 'utf8')).trim().split('\n').map(line => JSON.parse(line)) }
    catch (cause) { if (cause.code === 'ENOENT') return []; throw cause }
  }
  return { root, storage, models, invoke, calls }
}

test('integration reads the selected public owner at the actual data base before configuring models', async t => {
  const value = await fixture(t, 'current')
  await value.invoke()
  assert.deepEqual(await value.calls(), [
    { call: 'read', root: value.storage },
    { call: 'configure', root: value.storage, mode: 'custom', models: value.models },
  ])
  await assert.rejects(fs.access(value.models))
  await assert.rejects(fs.access(value.storage))
})

test('an older public owner without the required reader fails before configuration', async t => {
  const value = await fixture(t, 'missing')
  await assert.rejects(value.invoke(), cause => /preserved model-root reader is unavailable/.test(cause.stderr))
  assert.deepEqual(await value.calls(), [])
  await assert.rejects(fs.access(value.models))
})

test('selected-record refusal keeps its original cause and never falls through to configuration', async t => {
  const value = await fixture(t, 'refused')
  const original = path.join(value.root, 'untouched-control')
  await fs.writeFile(original, 'original bytes\r\n')
  await assert.rejects(value.invoke(), cause =>
    /original owner cause/.test(cause.stderr) && /selected control refused/.test(cause.stderr))
  assert.deepEqual(await value.calls(), [{ call: 'read', root: value.storage }])
  assert.equal(await fs.readFile(original, 'utf8'), 'original bytes\r\n')
  await assert.rejects(fs.access(value.models))
})
