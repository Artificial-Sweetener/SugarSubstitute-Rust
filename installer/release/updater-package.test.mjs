// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Exercise real file assembly and the shared archive encoder with a fake external PE inspector. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { gunzipSync } from 'node:zlib'
import { packageUpdater } from './updater-package.mjs'
import { parseImage } from './windows-imports.mjs'

const digest = bytes => createHash('sha256').update(bytes).digest('hex')
const platform = 'windows-x86_64'
const image = (imports = [], machine = 'AMD64 (0x8664)') => `Format: COFF-x86-64\nImageFileHeader {\n  Machine: IMAGE_FILE_MACHINE_${machine}\n}\nImageOptionalHeader {\n  ImportTableRVA: ${imports.length ? '0x1000' : '0x0'}\n  DelayImportDescriptorRVA: 0x0\n  ExportTableRVA: 0x0\n}\n` + imports.map(name => `Import {\n  Name: ${name}\n}\n`).join('')
const inspect = async file => parseImage(await fs.readFile(file, 'utf8'), platform)

async function fixture(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-engine-package-'))
  t.after(async () => {
    assert.equal(path.dirname(await fs.realpath(root)), await fs.realpath(os.tmpdir()))
    await fs.rm(root, { recursive: true })
  })
  const files = new Map()
  for (const [name, bytes] of [
    ['sugarsubstitute-updater.exe', image(['kernel32.dll', 'runtime.dll'])],
    ['sugarsubstitute_updater.pdb', 'inert engine symbols'],
    ['sugarsubstitute-updater.json', '{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]}\n'],
  ]) {
    const file = path.join(root, name)
    await fs.writeFile(file, bytes)
    files.set(name, file)
  }
  const runtime = path.join(root, 'runtime.dll'), extra = path.join(root, 'extra.dll')
  await fs.writeFile(runtime, image(['extra.dll']))
  await fs.writeFile(extra, image(['kernel32.dll', 'runtime.dll']))
  return { root, runtime, extra, selection: { component: 'updater', files }, output: path.join(root, 'package') }
}

test('engine assembly closes recursive imports and cycles and seals the exact shared package format', async t => {
  const f = await fixture(t)
  await fs.writeFile(f.selection.files.get('sugarsubstitute-updater.exe'), image(['runtime.dll']).replace('  Name: runtime.dll\n', '  Name: runtime.dll\n  Symbol: RequiredFunction (0)\n  Symbol:  (7)\n'))
  await fs.writeFile(f.runtime, image(['extra.dll']).replace('ExportTableRVA: 0x0', 'ExportTableRVA: 0x3000') + 'Export {\n  Name: RequiredFunction\n  Ordinal: 1\n}\nExport {\n  Name: \n  Ordinal: 7\n}\n')
  const report = await packageUpdater(f.selection, [f.runtime, f.extra], f.output, 'engine-1', inspect)
  const manifest = await fs.readFile(path.join(f.output, 'setup-manifest.json'))
  assert.equal(report.manifestSha256, digest(manifest))
  assert.deepEqual(report.dependencies.map(item => item.name).sort(), ['extra.dll', 'runtime.dll', 'sugarsubstitute-updater.exe'])
  const inventory = JSON.parse(manifest)
  assert.equal(inventory.files.length, 5)
  for (const entry of inventory.files) {
    const bytes = await fs.readFile(path.join(f.output, entry.path))
    assert.equal(entry.bytes, bytes.length)
    assert.equal(entry.sha256, digest(bytes))
  }
  const output = path.join(f.root, 'engine.ssupdate')
  const sealed = spawnSync(process.execPath, [fileURLToPath(new URL('../seal.mjs', import.meta.url)), '--update', f.output, output], { windowsHide: true, encoding: 'utf8', timeout: 10000 })
  assert.equal(sealed.status, 0, sealed.stderr)
  const archive = await fs.readFile(output)
  const footer = archive.subarray(-24), compressed = Number(footer.readBigUInt64LE())
  assert.equal(footer.subarray(16).toString(), 'SSSETUP1')
  const expected = Buffer.concat(await Promise.all(inventory.files.map(entry => fs.readFile(path.join(f.output, entry.path)))))
  assert.deepEqual(gunzipSync(archive.subarray(0, compressed)), expected)
  assert.deepEqual(archive.subarray(compressed, -24), manifest)
  await assert.rejects(packageUpdater(f.selection, [f.runtime, f.extra], f.output, 'engine-1', inspect), { code: 'EEXIST' })
})

test('missing, unused, duplicate, system and wrong-architecture runtime inputs never publish a manifest', async t => {
  for (const mode of ['missing', 'unused', 'duplicate', 'system', 'architecture', 'linked', 'entrypoint', 'ordinal']) {
    const f = await fixture(t)
    let inputs = [f.runtime, f.extra]
    if (mode === 'missing') inputs.pop()
    if (mode === 'duplicate') inputs.push(f.runtime)
    if (mode === 'system') inputs.push(path.join(f.root, 'kernel32.dll'))
    if (mode === 'unused') {
      const unused = path.join(f.root, 'unused.dll')
      await fs.writeFile(unused, image())
      inputs.push(unused)
    }
    if (mode === 'architecture') await fs.writeFile(f.extra, image([], 'ARM64 (0xAA64)'))
    if (mode === 'entrypoint') await fs.writeFile(f.runtime, image(['extra.dll']).replace('  Name: extra.dll\n', '  Name: extra.dll\n  Symbol: MissingFunction (0)\n'))
    if (mode === 'ordinal') await fs.writeFile(f.runtime, image(['extra.dll']).replace('  Name: extra.dll\n', '  Name: extra.dll\n  Symbol:  (7)\n'))
    if (mode === 'linked') {
      const linked = path.join(f.root, 'linked')
      await fs.symlink(f.root, linked, 'junction')
      inputs[0] = path.join(linked, 'runtime.dll')
    }
    await assert.rejects(packageUpdater(f.selection, inputs, f.output, 'engine-1', inspect))
    await assert.rejects(fs.stat(path.join(f.output, 'setup-manifest.json')), { code: 'ENOENT' })
    assert.ok((await fs.stat(f.runtime)).size > 0)
  }
})

test('application roles cannot be smuggled into an independent engine package', async t => {
  for (const mode of ['application', 'extra', 'missing']) {
    const f = await fixture(t)
    if (mode === 'application') f.selection.component = 'application'
    if (mode === 'extra') f.selection.files.set('sugarsubstitute.exe', f.runtime)
    if (mode === 'missing') f.selection.files.delete('sugarsubstitute-updater.json')
    await assert.rejects(packageUpdater(f.selection, [f.runtime, f.extra], f.output, 'engine-1', inspect))
    await assert.rejects(fs.stat(path.join(f.output, 'setup-manifest.json')), { code: 'ENOENT' })
  }
})

test('linked output ancestry is rejected before creating package content', async t => {
  const f = await fixture(t)
  const other = path.join(f.root, 'other'), linked = path.join(f.root, 'linked')
  await fs.mkdir(other)
  await fs.symlink(other, linked, process.platform === 'win32' ? 'junction' : 'dir')
  await assert.rejects(packageUpdater(f.selection, [f.runtime, f.extra], path.join(linked, 'package'), 'engine-1', inspect))
  assert.deepEqual(await fs.readdir(other), [])
})

test('forwarded exports resolve to a real entry point and forwarding cycles are rejected', async t => {
  for (const cycle of [false, true]) {
    const f = await fixture(t)
    const exported = (name, forward) => image().replace('ExportTableRVA: 0x0', 'ExportTableRVA: 0x3000') + `Export {\n  Name: ${name}\n  Ordinal: 1\n${forward ? `  ForwardedTo: ${forward}\n` : ''}}\n`
    await fs.writeFile(f.selection.files.get('sugarsubstitute-updater.exe'), image(['runtime.dll']).replace('  Name: runtime.dll\n', '  Name: runtime.dll\n  Symbol: First (0)\n'))
    await fs.writeFile(f.runtime, exported('First', 'extra.Second'))
    await fs.writeFile(f.extra, exported('Second', cycle ? 'runtime.First' : undefined))
    const result = packageUpdater(f.selection, [f.runtime, f.extra], f.output, 'engine-1', inspect)
    if (cycle) {
      await assert.rejects(result, /does not resolve/)
      await assert.rejects(fs.stat(path.join(f.output, 'setup-manifest.json')), { code: 'ENOENT' })
    } else assert.equal((await result).files.length, 5)
  }
})
