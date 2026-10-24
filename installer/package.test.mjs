// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Package the producing application's exact declaration without inventing its capabilities. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { packageApplication } from './release/application-package.mjs'

const script = fileURLToPath(new URL('./package.mjs', import.meta.url))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')
async function fixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-application-contract-'))
  const app = path.join(root, 'app'), cubes = path.join(root, 'cubes'), build = path.join(root, 'build')
  for (const [base, files] of [
    [app, ['sugarsubstitute.exe', 'sugarsubstitute.pdb', 'sugarsubstitute_crashpad_client.dll', 'sugarsubstitute_crashpad_client.pdb', 'crashpad_handler.exe', 'crashpad_handler.pdb', 'sugarsubstitute-release-v1.manifest', 'sugarsubstitute-release-v1.sig', 'sugarsubstitute-launcher.exe', 'sugarsubstitute_launcher.pdb', 'sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb']],
    [cubes, ['__init__.py', 'prestartup_script.py', 'load_model_roots.py', 'pyproject.toml', 'requirements.txt', 'LICENSE', 'sugarcubes/module.py', 'sugarcubes_model_roots/module.py', 'web/module.js']],
    [build, ['worker/package.json', 'electron/electron.exe']],
  ]) {
    for (const file of files) {
      await fs.mkdir(path.dirname(path.join(base, file)), { recursive: true })
      await fs.writeFile(path.join(base, file), 'inert package fixture')
    }
  }
  await fs.writeFile(path.join(app, 'sugarsubstitute-updater.json'), '{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]}\n')
  await fs.writeFile(path.join(app, 'sugarsubstitute-installation-activation.json'), '{"schema_version":1,"authorization_schemas":[1],"engine_release":"1","engine_protocol":1}\n')
  const runtime = path.join(root, 'vcruntime140.dll')
  await fs.writeFile(runtime, 'inert runtime fixture')
  return { root, app, cubes, build, runtimeFiles: [runtime] }
}
async function inspect(file) {
  const name = path.basename(file)
  if (name === 'vcruntime140.dll') return { imports: [], exports: [{ name: 'RuntimeFunction', ordinal: 1 }] }
  if (['sugarsubstitute.exe', 'sugarsubstitute-launcher.exe', 'sugarsubstitute-updater.exe'].includes(name)) return { imports: [], exports: [] }
  const imports = [{ library: 'vcruntime140.dll', symbols: ['RuntimeFunction'], ordinals: [] }]
  if (name === 'crashpad_handler.exe') imports.push({ library: 'sugarsubstitute_crashpad_client.dll', symbols: ['NativeClientFunction'], ordinals: [] })
  return { imports, exports: name === 'sugarsubstitute_crashpad_client.dll' ? [{ name: 'NativeClientFunction', ordinal: 1 }] : [] }
}
function assemble(f, output, inspector = inspect) {
  return packageApplication({ ...f, output, release: 'fixture' }, inspector)
}

test('packaging authenticates the producer declaration byte-for-byte, including future versions', async () => {
  const f = await fixture()
  const declaration = Buffer.from('{"schema_version":1,"installed_launch_protocols":[2]}\n')
  const launcher = Buffer.from('{"schema_version":1,"installation_schemas":[1,2],"installed_launch_protocol":2}\n')
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-application.json'), declaration)
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-launcher.json'), launcher)
  const output = path.join(f.root, 'output')
  const result = await assemble(f, output)
  const manifest = JSON.parse(await fs.readFile(path.join(output, 'setup-manifest.json'), 'utf8'))
  assert.equal(result.dependencies.length, 6)
  assert.equal(result.dependencies.filter(value => value.name === 'vcruntime140.dll').length, 1)
  const runtime = manifest.files.find(file => file.path === 'app/vcruntime140.dll')
  assert.equal(runtime.sha256, hash(await fs.readFile(f.runtimeFiles[0])))
  assert.deepEqual(await fs.readFile(path.join(output, runtime.path)), await fs.readFile(f.runtimeFiles[0]))
  const entry = manifest.files.find(file => file.path === 'app/sugarsubstitute-application.json')
  assert.deepEqual(entry, { path: 'app/sugarsubstitute-application.json', bytes: declaration.length, sha256: hash(declaration) })
  assert.deepEqual(await fs.readFile(path.join(output, entry.path)), declaration)
  for (const name of ['sugarsubstitute-launcher.exe', 'sugarsubstitute_launcher.pdb', 'sugarsubstitute-launcher.json', 'sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb', 'sugarsubstitute-updater.json', 'sugarsubstitute-installation-activation.json']) {
    const bytes = await fs.readFile(path.join(f.app, name))
    const item = manifest.files.find(file => file.path === `app/${name}`)
    assert.deepEqual(item, { path: `app/${name}`, bytes: bytes.length, sha256: hash(bytes) })
    assert.deepEqual(await fs.readFile(path.join(output, item.path)), bytes)
  }
})

test('packaging requires each producing launcher and updater executable, symbols and declaration', async () => {
  for (const name of ['sugarsubstitute-launcher.exe', 'sugarsubstitute_launcher.pdb', 'sugarsubstitute-launcher.json', 'sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb', 'sugarsubstitute-updater.json', 'sugarsubstitute-installation-activation.json']) {
    const f = await fixture(), output = path.join(f.root, 'output')
    await fs.writeFile(path.join(f.app, 'sugarsubstitute-application.json'), '{"schema_version":1,"installed_launch_protocols":[1]}')
    await fs.writeFile(path.join(f.app, 'sugarsubstitute-launcher.json'), '{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":1}')
    await fs.unlink(path.join(f.app, name))
    await assert.rejects(assemble(f, output))
    await assert.rejects(fs.stat(path.join(output, 'setup-manifest.json')), { code: 'ENOENT' })
  }
})

test('packaging refuses to guess launch support when the producing application has no declaration', async () => {
  const f = await fixture(), output = path.join(f.root, 'output')
  await assert.rejects(assemble(f, output))
  await assert.rejects(fs.stat(path.join(output, 'setup-manifest.json')), { code: 'ENOENT' })
})

test('native runtime requirements cannot be satisfied by a missing library or an Electron sibling', async () => {
  const f = await fixture(), output = path.join(f.root, 'output')
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-application.json'), '{}')
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-launcher.json'), '{}')
  await fs.copyFile(f.runtimeFiles[0], path.join(f.build, 'electron', 'vcruntime140.dll'))
  await assert.rejects(assemble({ ...f, runtimeFiles: [] }, output), /Native dependency is missing/)
  await assert.rejects(fs.stat(path.join(output, 'setup-manifest.json')), { code: 'ENOENT' })
})

test('an incompatible runtime never produces a completed application inventory', async () => {
  const f = await fixture(), output = path.join(f.root, 'output')
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-application.json'), '{}')
  await fs.writeFile(path.join(f.app, 'sugarsubstitute-launcher.json'), '{}')
  await assert.rejects(assemble(f, output, async file => path.basename(file) === 'vcruntime140.dll' ? { imports: [], exports: [] } : inspect(file)), /required entry points/)
  await assert.rejects(fs.stat(path.join(output, 'setup-manifest.json')), { code: 'ENOENT' })
})

test('the packaging CLI requires an explicit native inspection platform and tool', async () => {
  const f = await fixture(), output = path.join(f.root, 'output')
  const result = spawnSync(process.execPath, [script, f.app, f.cubes, output, 'fixture'], {
    encoding: 'utf8', windowsHide: true, timeout: 10000, env: { ...process.env, SUBSTITUTE_INSTALLER_BUILD: f.build },
  })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Usage:/)
  await assert.rejects(fs.stat(output), { code: 'ENOENT' })
})

test('bundled runtime DLLs cannot excuse a bootstrap dependency in supplied application binaries', async () => {
  for (const entry of ['sugarsubstitute.exe', 'sugarsubstitute-launcher.exe', 'sugarsubstitute-updater.exe']) {
    const f = await fixture(), output = path.join(f.root, 'output')
    await fs.writeFile(path.join(f.app, 'sugarsubstitute-application.json'), '{}')
    await fs.writeFile(path.join(f.app, 'sugarsubstitute-launcher.json'), '{}')
    await assert.rejects(assemble(f, output, async file => path.basename(file) === entry
      ? { imports: [{ library: 'vcruntime140.dll', symbols: ['RuntimeFunction'], ordinals: [] }], exports: [] }
      : inspect(file)), /Native entry requires a separately installed library/)
    await assert.rejects(fs.stat(path.join(output, 'setup-manifest.json')), { code: 'ENOENT' })
  }
})
