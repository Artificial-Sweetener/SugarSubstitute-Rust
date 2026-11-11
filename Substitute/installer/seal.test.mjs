// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Real local packaging CLI preserves one archive format for setup and update delivery. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { gunzipSync } from 'node:zlib'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('./seal.mjs', import.meta.url))
const hash = bytes => createHash('sha256').update(bytes).digest('hex')

test('installer and update outputs contain identical authenticated inventory and expanded bytes', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-package-'))
  const bundle = path.join(root, 'bundle')
  await fs.mkdir(path.join(bundle, 'app'), { recursive: true })
  const payload = Buffer.from('inert packaging fixture')
  await fs.writeFile(path.join(bundle, 'app/sugarsubstitute.exe'), payload)
  const manifest = Buffer.from(JSON.stringify({ schema_version: 1, release_id: 'fixture', files: [
    { path: 'app/sugarsubstitute.exe', bytes: payload.length, sha256: hash(payload) },
  ] }))
  await fs.writeFile(path.join(bundle, 'setup-manifest.json'), manifest)
  const stub = path.join(root, 'stub.exe')
  await fs.writeFile(stub, 'inert executable envelope')
  for (const update of [true, false]) {
    const output = path.join(root, update ? 'package.ssupdate' : 'setup.exe')
    const args = [script, update ? '--update' : stub, bundle, output]
    const result = spawnSync(process.execPath, args, { encoding: 'utf8', windowsHide: true, timeout: 10000 })
    assert.equal(result.error, undefined)
    assert.equal(result.status, 0, result.stderr)
    const report = JSON.parse(result.stdout)
    const bytes = await fs.readFile(output)
    assert.equal(report.sha256, hash(bytes))
    assert.equal(report.manifestSha256, hash(manifest))
    assert.equal(report.bytes, bytes.length)
    assert.equal(report[update ? 'package' : 'executable'], output)
    const footer = bytes.subarray(-24)
    assert.equal(footer.subarray(16).toString(), 'SSSETUP1')
    const compressed = Number(footer.readBigUInt64LE())
    const manifestBytes = Number(footer.readBigUInt64LE(8))
    const offset = bytes.length - 24 - manifestBytes - compressed
    assert.equal(offset, update ? 0 : (await fs.stat(stub)).size)
    assert.deepEqual(gunzipSync(bytes.subarray(offset, offset + compressed)), payload)
    assert.deepEqual(bytes.subarray(offset + compressed, -24), manifest)
    const duplicate = spawnSync(process.execPath, args, { encoding: 'utf8', windowsHide: true, timeout: 10000 })
    assert.notEqual(duplicate.status, 0)
    assert.deepEqual(await fs.readFile(output), bytes)
  }
  const archive = path.join(root, 'package.ssupdate')
  const signedBytes = await fs.readFile(archive)
  const output = path.join(root, 'attached.exe')
  const result = spawnSync(process.execPath, [script, '--attach', stub, archive, hash(signedBytes), output], {
    encoding: 'utf8', windowsHide: true, timeout: 10000,
  })
  assert.equal(result.error, undefined)
  assert.equal(result.status, 0, result.stderr)
  const report = JSON.parse(result.stdout)
  const bytes = await fs.readFile(output)
  assert.deepEqual(bytes, Buffer.concat([await fs.readFile(stub), signedBytes]))
  assert.equal(report.packageSha256, hash(signedBytes))
  assert.equal(report.packageBytes, signedBytes.length)
  assert.equal(report.sha256, hash(bytes))
  assert.equal(report.manifestSha256, hash(manifest))
})

test('changed source bytes cannot produce a completed update package', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-package-rejection-'))
  await fs.mkdir(path.join(root, 'app'))
  await fs.writeFile(path.join(root, 'app/file'), 'changed')
  await fs.writeFile(path.join(root, 'setup-manifest.json'), JSON.stringify({ schema_version: 1, release_id: 'fixture', files: [
    { path: 'app/file', bytes: 7, sha256: hash('correct') },
  ] }))
  const output = path.join(root, 'rejected.ssupdate')
  const result = spawnSync(process.execPath, [script, '--update', root, output], { encoding: 'utf8', windowsHide: true, timeout: 10000 })
  assert.equal(result.error, undefined)
  assert.notEqual(result.status, 0)
  await assert.rejects(fs.stat(output), { code: 'ENOENT' })
})
