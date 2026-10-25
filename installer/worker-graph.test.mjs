// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { requireHeadlessGraph, requireWorkerPayload } from './worker-graph.mjs'
import { runtimeArchive, runtime, runtimeFlags } from './runtime.mjs'

test('worker graph rejects Electron, external code and incomplete or mixed runtime payloads', () => {
  const graph = (name, dependency) => ({ inputs: { [name]: {} }, outputs: { bundle: { imports: [{ path: dependency, external: true }] } } })
  requireHeadlessGraph(graph('worker/main.ts', 'node:http'))
  requireHeadlessGraph(graph('worker/main.ts', '7zip-bin'))
  for (const name of ['node_modules/electron/index.js', 'node_modules/@electron/get/index.js', 'C:\\x\\Electron\\index.js']) {
    assert.throws(() => requireHeadlessGraph(graph(name, 'node:fs')), /Electron/)
  }
  for (const name of ['electron', 'unreviewed-package', './external.cjs']) assert.throws(() => requireHeadlessGraph(graph('worker/main.ts', name)), /Unapproved/)
  const roles = ['worker/package.json', 'worker/out/main/index.cjs', 'worker/runtime/node.exe', 'worker/runtime/LICENSE']
  requireWorkerPayload(roles)
  for (const role of roles) assert.throws(() => requireWorkerPayload(roles.filter(name => name !== role)), /missing/)
  for (const role of ['electron/electron.exe', 'worker/node_modules/electron/index.js', 'worker/runtime/resources.pak', 'worker/runtime/NODE.exe']) {
    assert.throws(() => requireWorkerPayload([...roles, role]), /Unapproved/)
  }
})

test('standalone runtimes have explicit official archive pins and restricted launch options', () => {
  for (const [platform, arch] of Object.keys(runtime.archives).map(key => key.split('-'))) {
    const pin = runtimeArchive(platform, arch)
    assert.match(pin.url, /^https:\/\/nodejs\.org\/dist\/v24\./)
    assert.match(pin.sha256, /^[a-f0-9]{64}$/)
  }
  assert.throws(() => runtimeArchive('win32', 'x86'), /Unsupported/)
  assert.deepEqual(runtimeFlags, ['--use-system-ca', '--use-env-proxy', '--no-global-search-paths'])
})
