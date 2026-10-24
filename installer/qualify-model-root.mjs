// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Qualify the adapter against the selected SugarCubes release's actual public owner. */
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { configureModelRoot } from './worker/model-root.mjs'

const [extension, python, workspace] = process.argv.slice(2)
if (process.argv.length !== 5 || [extension, python, workspace].some(value => !path.isAbsolute(value))) {
  throw new Error('Usage: node installer/qualify-model-root.mjs <SugarCubes-release> <python> <new-workspace>')
}
await fs.mkdir(workspace)
const comfy = path.join(workspace, 'ComfyUI')
await fs.mkdir(comfy)
const configure = models => promisify(execFile)(python,
  ['-B', '-c', configureModelRoot, extension, comfy, models], { windowsHide: true })
const models = path.join(workspace, 'models')
await configure(models)
assert.deepEqual(await fs.readdir(models), [])
const state = path.join(comfy, '.substitute')
const record = path.join(state, 'model_root.json')
assert.equal(JSON.parse(await fs.readFile(record, 'utf8')).modelRoot, models)
await fs.writeFile(path.join(state, 'user-note'), 'preserved')
const replacement = path.join(workspace, 'existing models')
await fs.mkdir(replacement)
await fs.writeFile(path.join(replacement, 'owned-model.safetensors'), 'preserve model bytes')
await configure(replacement)
assert.equal(JSON.parse(await fs.readFile(record, 'utf8')).modelRoot, replacement)
assert.equal(await fs.readFile(path.join(replacement, 'owned-model.safetensors'), 'utf8'), 'preserve model bytes')
assert.equal(await fs.readFile(path.join(state, 'user-note'), 'utf8'), 'preserved')
const unknown = '{"schemaVersion":999}'
await fs.writeFile(record, unknown)
await assert.rejects(configure(models), /Unsupported model-root configuration schema/)
assert.equal(await fs.readFile(record, 'utf8'), unknown)
await fs.writeFile(path.join(workspace, 'receipt.json'), JSON.stringify({
  schema_version: 1, passed: true, actual_sugarcubes_public_api: true,
  model_bytes_preserved: true, unknown_configuration_preserved: true, no_model_downloads: true,
}, null, 2))
console.log('Actual SugarCubes model-root integration passed')
