// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Cargo's actual artifact identities, not guessed target paths, define application assembly. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { nativeTargets, selectArtifacts, assembleArtifacts } from './application-artifacts.mjs'
import { engineBuildFile } from './engine-identity.mjs'

async function fixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-cargo-artifacts-'))
  const workspace = path.join(root, 'workspace'), target = path.join(root, 'target')
  const out = path.join(target, 'debug', 'build', 'actual-package-id', 'out')
  await fs.mkdir(out, { recursive: true })
  const messages = [{ reason: 'build-script-executed', package_id: 'application-package', out_dir: out }]
  for (const name of ['sugarsubstitute', 'sugarsubstitute-launcher', 'sugarsubstitute-updater']) {
    const executable = path.join(target, 'debug', `${name}.exe`)
    const pdb = path.join(target, 'debug', `${name.replaceAll('-', '_')}.pdb`)
    await fs.writeFile(executable, `inert executable ${name}`)
    await fs.writeFile(pdb, `inert symbols ${name}`)
    messages.push({ reason: 'compiler-artifact', package_id: 'application-package', manifest_path: path.join(workspace, 'crates', 'sugarsubstitute', 'Cargo.toml'), target: { name, kind: ['bin'] }, executable, filenames: [executable, pdb] })
  }
  const contract = Buffer.from('{"schema_version":1,"installed_launch_protocols":[1]}\n')
  await fs.writeFile(path.join(out, engineBuildFile), '{"schema_version":1,"release":"17","protocol":1}\n')
  await fs.writeFile(path.join(out, 'sugarsubstitute-application.json'), contract)
  await fs.writeFile(path.join(out, 'sugarsubstitute-launcher.json'), '{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":1}\n')
  await fs.writeFile(path.join(out, 'sugarsubstitute-installation-activation.json'), '{"schema_version":1,"authorization_schemas":[1],"engine_release":"17","engine_protocol":1}\n')
  await fs.writeFile(path.join(out, 'sugarsubstitute-updater.json'), '{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]}\n')
  messages.push({ reason: 'build-finished', success: true })
  return { root, workspace, target, out, messages, contract }
}

test('same-invocation executable, symbols and declaration assemble into a fresh complete receipt', async () => {
  const f = await fixture(), output = path.join(f.root, 'output')
  const selected = selectArtifacts(f.messages, f.workspace, f.target, 'application')
  const receipt = await assembleArtifacts(selected, output)
  assert.deepEqual(receipt.engine, { schema_version: 1, release: '17', protocol: 1 })
  const identityBytes = await fs.readFile(path.join(output, receipt.engine_build.name))
  assert.equal(receipt.engine_build.sha256, createHash('sha256').update(identityBytes).digest('hex'))
  assert.deepEqual(identityBytes, await fs.readFile(selected.engineBuild))
  assert.equal(receipt.files.length, 10)
  for (const file of receipt.files) {
    const bytes = await fs.readFile(path.join(output, file.name))
    assert.equal(file.bytes, bytes.length)
    assert.equal(file.sha256, createHash('sha256').update(bytes).digest('hex'))
    assert.deepEqual(bytes, await fs.readFile(selected.files.get(file.name)))
  }
  await assert.rejects(assembleArtifacts(selected, output), { code: 'EEXIST' })
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(output, 'application-build.json'), 'utf8')), receipt)
})

test('setup retains its actual compiled distribution and has an independent target inventory', async () => {
  const f = await fixture()
  const messages = [f.messages[0], structuredClone(f.messages[1]), f.messages.at(-1)]
  const artifact = messages[1]
  artifact.target.name = 'sugarsubstitute-setup'
  artifact.executable = path.join(f.target, 'debug', 'sugarsubstitute-setup.exe')
  artifact.filenames = [artifact.executable, path.join(f.target, 'debug', 'sugarsubstitute_setup.pdb')]
  for (const file of artifact.filenames) await fs.writeFile(file, 'inert setup output')
  const distribution = path.join(f.out, 'installer-distribution.json')
  const selected = selectArtifacts(messages, f.workspace, f.target, 'setup')
  assert.deepEqual(nativeTargets('setup'), ['sugarsubstitute-setup'])
  nativeTargets('setup').push('foreign')
  assert.deepEqual(nativeTargets('setup'), ['sugarsubstitute-setup'])
  assert.equal(selected.engineBuild, undefined)
  await assert.rejects(assembleArtifacts(selected, path.join(f.root, 'missing')), { code: 'ENOENT' })
  await fs.writeFile(distribution, '')
  await assert.rejects(assembleArtifacts(selected, path.join(f.root, 'disabled')), /absent or oversized/)
  const oversized = await fs.open(distribution, 'r+')
  try { await oversized.truncate(4 * 1024 ** 2 + 1) } finally { await oversized.close() }
  await assert.rejects(assembleArtifacts(selected, path.join(f.root, 'oversized')), /absent or oversized/)
  const bytes = Buffer.from('{"schema_version":1,"fixture":"compiled distribution bytes"}\n')
  await fs.writeFile(distribution, bytes)
  const output = path.join(f.root, 'setup')
  const receipt = await assembleArtifacts(selected, output)
  assert.equal(receipt.files.length, 3)
  assert.equal(receipt.engine, undefined)
  assert.deepEqual(await fs.readFile(path.join(output, 'installer-distribution.json')), bytes)
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(output, 'setup-build.json'), 'utf8')), receipt)
  assert.throws(() => selectArtifacts(messages, f.workspace, f.target, 'application'))
})

test('failed, duplicate and mixed-package compiler outputs cannot authorize assembly', async () => {
  const f = await fixture()
  for (const mutate of [
    messages => { messages.at(-1).success = false },
    messages => { messages.push(messages.at(-1)) },
    messages => { messages[2].package_id = 'foreign-package' },
    messages => { messages.push(messages[1]) },
    messages => { messages.splice(0, 1) },
    messages => { messages.splice(3, 1) },
    messages => { messages[3].package_id = 'foreign-updater' },
    messages => { messages[1].manifest_path = path.join(f.root, 'foreign', 'Cargo.toml') },
    messages => { messages[2].target.name = 'sugarsubstitute' },
  ]) {
    const messages = structuredClone(f.messages)
    mutate(messages)
    assert.throws(() => selectArtifacts(messages, f.workspace, f.target, 'application'))
  }
})

test('artifacts outside the explicit target tree and missing native symbols fail closed', async () => {
  const f = await fixture()
  for (const mutate of [
    messages => { messages[0].out_dir = f.root },
    messages => { messages[1].filenames.pop() },
    messages => { messages[1].executable = path.join(f.root, 'sugarsubstitute.exe') },
    messages => { messages[1].filenames.push(messages[1].filenames[1]) },
  ]) {
    const messages = structuredClone(f.messages)
    mutate(messages)
    assert.throws(() => selectArtifacts(messages, f.workspace, f.target, 'application'))
  }
})

test('absent producing declaration cannot leave a completed artifact receipt', async () => {
  for (const name of ['sugarsubstitute-application.json', 'sugarsubstitute-launcher.json', 'sugarsubstitute-updater.json', 'sugarsubstitute-installation-activation.json', engineBuildFile]) {
    const f = await fixture(), output = path.join(f.root, 'output')
    const selected = selectArtifacts(f.messages, f.workspace, f.target, 'application')
    await fs.unlink(path.join(f.out, name))
    await assert.rejects(assembleArtifacts(selected, output), { code: 'ENOENT' })
    await assert.rejects(fs.stat(path.join(output, 'application-build.json')), { code: 'ENOENT' })
  }
})

test('an independent updater build selects only its executable, symbols and producing contract', async () => {
  const f = await fixture()
  const messages = f.messages.filter(message => message.reason !== 'compiler-artifact' || message.target.name === 'sugarsubstitute-updater')
  const selected = selectArtifacts(messages, f.workspace, f.target, 'updater')
  assert.deepEqual([...selected.files.keys()].sort(), ['sugarsubstitute-updater.exe', 'sugarsubstitute-updater.json', 'sugarsubstitute_updater.pdb'].sort())
  const output = path.join(f.root, 'engine')
  const receipt = await assembleArtifacts(selected, output)
  assert.equal(receipt.files.length, 3)
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(output, 'updater-build.json'), 'utf8')), receipt)
  assert.throws(() => selectArtifacts(messages, f.workspace, f.target, 'application'))
  assert.throws(() => selectArtifacts(messages, f.workspace, f.target, 'unknown'))
})
