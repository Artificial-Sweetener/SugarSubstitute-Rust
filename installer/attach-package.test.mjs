// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Exact signed bytes survive executable attachment; failed attachment has no success receipt. */
import { test } from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { attachPackage } from './attach-package.mjs'

const hash = bytes => createHash('sha256').update(bytes).digest('hex')

async function fixture(body) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-attachment-'))
  try {
    const stub = path.join(root, 'stub.exe')
    const archive = path.join(root, 'package.ssupdate')
    const output = path.join(root, 'setup.exe')
    const compressed = Buffer.alloc(131071, 0x61)
    const manifest = Buffer.alloc(131073, 0x62)
    const footer = Buffer.alloc(24)
    footer.writeBigUInt64LE(BigInt(compressed.length), 0)
    footer.writeBigUInt64LE(BigInt(manifest.length), 8)
    footer.write('SSSETUP1', 16)
    const bytes = Buffer.concat([compressed, manifest, footer])
    await fs.writeFile(stub, 'inert executable')
    await fs.writeFile(archive, bytes)
    await body({ stub, archive, output, bytes, manifest })
  } finally {
    await fs.rm(root, { recursive: true })
  }
}

test('attachment streams the exact package and hashes inventory across read boundaries', async () => {
  await fixture(async ({ stub, archive, output, bytes, manifest }) => {
    const report = await attachPackage(stub, archive, hash(bytes), output)
    const actual = await fs.readFile(output)
    assert.deepEqual(actual, Buffer.concat([await fs.readFile(stub), bytes]))
    assert.equal(report.sha256, hash(actual))
    assert.equal(report.packageSha256, hash(bytes))
    assert.equal(report.packageBytes, bytes.length)
    assert.equal(report.manifestSha256, hash(manifest))
    await assert.rejects(attachPackage(stub, archive, hash(bytes), output), { code: 'EEXIST' })
    assert.deepEqual(await fs.readFile(output), actual)
  })
})

test('digest failure preserves the supplied package and never returns a sealed receipt', async () => {
  await fixture(async ({ stub, archive, output, bytes }) => {
    await assert.rejects(attachPackage(stub, archive, '0'.repeat(64), output), /authentication failed/)
    assert.deepEqual(await fs.readFile(archive), bytes)
    assert.equal((await fs.readFile(stub)).toString(), 'inert executable')
  })
})

test('nested executable, invalid footer and oversized manifest fail before output creation', async () => {
  await fixture(async ({ stub, archive, output, bytes }) => {
    const oversized = Buffer.from(bytes)
    oversized.writeBigUInt64LE(1024n ** 2n + 1n, oversized.length - 16)
    for (const invalid of [Buffer.concat([Buffer.from('envelope'), bytes]), Buffer.from('truncated'), oversized]) {
      await fs.writeFile(archive, invalid)
      await assert.rejects(attachPackage(stub, archive, hash(invalid), output))
      await assert.rejects(fs.stat(output), { code: 'ENOENT' })
      assert.deepEqual(await fs.readFile(archive), invalid)
    }
  })
})
