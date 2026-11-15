// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createHash } from 'node:crypto'
import { newerRoots, publishedRelease, sealedPackage, writeReleaseInstallerDistribution } from './published-release.mjs'

const platform = 'windows-x86_64'
const descriptorName = `application-release-${platform}.json`
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')

/** A sealed package: compressed payload, inventory, then the footer naming both lengths. */
function sealed(payload, inventory) {
  const footer = Buffer.alloc(24)
  footer.writeBigUInt64LE(BigInt(payload.length), 0)
  footer.writeBigUInt64LE(BigInt(inventory.length), 8)
  footer.write('SSSETUP1', 16)
  return Buffer.concat([payload, inventory, footer])
}

/**
 * A repository folder publishing 0.2.0 at targets 3 and snapshot 4, beside the superseded
 * targets 2, with the shipped root 1 and a rotated root 2.
 */
async function fixture(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'substitute-published-'))
  t.after(() => fs.rm(root, { recursive: true, force: true }))
  const repository = path.join(root, 'repository')
  await fs.mkdir(repository)
  const inventory = Buffer.from('{"schema_version":1,"files":[]}')
  const bytes = sealed(Buffer.from('compressed payload'), inventory)
  const packageFile = path.join(root, 'package.ssupdate')
  await fs.writeFile(packageFile, bytes)
  const offer = {
    schema_version: 1, product: 'sugarsubstitute', channel: 'release', platform, component: 'application',
    sequence: 2 << 20, version: '0.2.0', package_sha256: sha256(bytes), package_bytes: bytes.length,
  }
  const files = {
    '1.root.json': '{"signed":{"_type":"root","version":1}}',
    '2.root.json': '{"signed":{"_type":"root","version":2}}\n',
    'timestamp.json': '{"signed":{"_type":"timestamp","meta":{"snapshot.json":{"version":4}},"version":9}}',
    '4.snapshot.json': '{"signed":{"_type":"snapshot","meta":{"targets.json":{"version":3}},"version":4}}',
    '3.targets.json': `{"signed":{"_type":"targets","targets":{"${descriptorName}":{},"${offer.package_sha256}.ssupdate":{}},"version":3}}`,
    '2.targets.json': '{"signed":{"_type":"targets","targets":{},"version":2}}',
    [descriptorName]: JSON.stringify(offer),
  }
  for (const [name, text] of Object.entries(files)) await fs.writeFile(path.join(repository, name), text)
  const distribution = path.join(root, 'update-distribution.json')
  await fs.writeFile(distribution, JSON.stringify({ schema_version: 4, repository: [...Buffer.alloc(32, 0x5a)] }))
  const rewrite = (name, change) => fs.writeFile(path.join(repository, name), JSON.stringify(change(JSON.parse(files[name]))))
  return {
    root, repository, files, offer, inventory, bytes, packageFile, rewrite,
    request: {
      repository, version: '0.2.0', platform, packageFile, output: path.join(root, 'installer-distribution.json'),
      trustedRoot: path.join(repository, '1.root.json'), distribution,
    },
  }
}

test('the published release is found the way an installation finds it', async t => {
  const { repository, offer } = await fixture(t)
  assert.deepEqual(await publishedRelease(repository, platform), {
    version: '0.2.0',
    timestamp: path.join(repository, 'timestamp.json'),
    snapshot: path.join(repository, '4.snapshot.json'),
    targets: path.join(repository, '3.targets.json'),
    descriptor: path.join(repository, descriptorName),
    package: { name: `${offer.package_sha256}.ssupdate`, sha256: offer.package_sha256, bytes: offer.package_bytes },
  })
})

test('the distribution carries the signed files exactly, the rotated roots and the package inventory digest', async t => {
  const { repository, files, inventory, bytes, request } = await fixture(t)
  const receipt = await writeReleaseInstallerDistribution(request)
  const written = await fs.readFile(request.output)
  const value = JSON.parse(written)
  assert.equal(value.schema_version, 1)
  assert.deepEqual(value.repository, [...Buffer.alloc(32, 0x5a)])
  assert.equal(value.channel, 'release')
  assert.equal(value.manifest_sha256, sha256(inventory))
  assert.equal(value.trusted_root, files['1.root.json'])
  assert.deepEqual(value.roots, [{ version: 2, metadata: files['2.root.json'] }])
  assert.equal(value.timestamp, files['timestamp.json'])
  assert.equal(value.snapshot, files['4.snapshot.json'])
  assert.equal(value.targets, files['3.targets.json'])
  assert.equal(value.descriptor, files[descriptorName])
  assert.deepEqual(receipt, {
    configuration: request.output, bytes: written.length, sha256: sha256(written), version: '0.2.0',
    package: `${sha256(bytes)}.ssupdate`, packageSha256: sha256(bytes), manifestSha256: sha256(inventory),
  })
  assert.deepEqual(await newerRoots(repository, path.join(repository, '2.root.json')), [])
})

test('an installer is built only for the release the repository publishes, from the package it offers', async t => {
  const { root, bytes, request } = await fixture(t)
  await assert.rejects(writeReleaseInstallerDistribution({ ...request, version: '0.1.9' }), /publishes 0\.2\.0, not 0\.1\.9/)
  const other = path.join(root, 'other.ssupdate')
  await fs.writeFile(other, sealed(Buffer.from('another payload!!!'), Buffer.from('{"schema_version":1,"files":[]}')))
  await assert.rejects(writeReleaseInstallerDistribution({ ...request, packageFile: other }), /not the one the update repository offers/)
  const truncated = path.join(root, 'truncated.ssupdate')
  await fs.writeFile(truncated, bytes.subarray(0, bytes.length - 1))
  await assert.rejects(writeReleaseInstallerDistribution({ ...request, packageFile: truncated }), /footer is invalid/)
  await assert.rejects(writeReleaseInstallerDistribution({ ...request, platform: 'windows-aarch64' }), /publishes no application release for windows-aarch64/)
  await assert.rejects(fs.stat(request.output), { code: 'ENOENT' })
})

test('a repository whose files do not lead to an application release is refused', async t => {
  const { repository, files, rewrite } = await fixture(t)
  const restore = name => fs.writeFile(path.join(repository, name), files[name])
  await rewrite('timestamp.json', value => ({ signed: { ...value.signed, meta: {} } }))
  await assert.rejects(publishedRelease(repository, platform), /timestamp\.json names no metadata version/)
  await rewrite('timestamp.json', value => ({ signed: { ...value.signed, meta: { 'snapshot.json': { version: 7 } } } }))
  await assert.rejects(publishedRelease(repository, platform), /no usable 7\.snapshot\.json/)
  await restore('timestamp.json')
  for (const change of [{ product: 'other' }, { channel: 'canary' }, { component: 'updater' }, { version: '0.2' }, { package_sha256: 'A'.repeat(64) }, { package_bytes: 1 }]) {
    await rewrite(descriptorName, value => ({ ...value, ...change }))
    await assert.rejects(publishedRelease(repository, platform), /does not offer an application release/)
  }
  await restore(descriptorName)
  await fs.rm(path.join(repository, descriptorName))
  await assert.rejects(publishedRelease(repository, platform), new RegExp(`no usable ${descriptorName.replaceAll('.', '\\.')}`))
})

test('a sealed package reports its own length, digest and inventory digest', async t => {
  const { bytes, inventory, packageFile } = await fixture(t)
  assert.deepEqual(await sealedPackage(packageFile), { bytes: bytes.length, sha256: sha256(bytes), manifestSha256: sha256(inventory) })
  await assert.rejects(sealedPackage('relative.ssupdate'), /absolute path/)
})
