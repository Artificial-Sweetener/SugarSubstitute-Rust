// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import test from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createHash, createPublicKey, generateKeyPairSync } from 'node:crypto'
import { anchorKey, generateReleaseKey, keyId, publicBlob, signManifest, verifyManifest, writeReleaseKey } from './release-signature.mjs'
import { crashHandlerFiles, manifestFile, signRelease, signatureFile } from './sign-release.mjs'
import { engineBuildFile } from './engine-identity.mjs'

const key = generateReleaseKey()

test('the public blob is the exact CNG RSA-3072 shape the application compiles in', () => {
  const blob = key.publicBlob
  assert.equal(blob.length, 411)
  assert.equal(blob.subarray(0, 24).toString('hex'), '52534131000c000003000000800100000000000000000000')
  assert.equal(blob.subarray(24, 27).toString('hex'), '010001')
  const jwk = createPublicKey(key.privatePem).export({ format: 'jwk' })
  assert.deepEqual(blob.subarray(27), Buffer.from(jwk.n, 'base64url'))
  assert.deepEqual(anchorKey(blob).export({ format: 'jwk' }), jwk)
  assert.equal(keyId(blob), createHash('sha256').update(blob).digest('hex'))
})

test('a signature verifies under its anchor and nowhere else', () => {
  const manifest = Buffer.from('SSRELEASE-K1-v2\0 release manifest bytes')
  const signature = signManifest(key.privatePem, manifest, key.publicBlob)
  assert.equal(signature.length, 384)
  assert.ok(verifyManifest(manifest, signature, key.publicBlob))
  const tampered = Buffer.from(manifest)
  tampered[3] ^= 1
  assert.equal(verifyManifest(tampered, signature, key.publicBlob), false)
  assert.equal(verifyManifest(manifest, signature, generateReleaseKey().publicBlob), false)
})

test('signing refuses a key the committed anchor does not name, or another key shape', () => {
  const manifest = Buffer.from('manifest')
  assert.throws(() => signManifest(generateReleaseKey().privatePem, manifest, key.publicBlob), /does not match the committed release anchor/)
  const small = generateKeyPairSync('rsa', { modulusLength: 2048 }).privateKey.export({ type: 'pkcs8', format: 'pem' })
  assert.throws(() => signManifest(small, manifest, key.publicBlob), /RSA-3072/)
  assert.throws(() => signManifest('not a key', manifest, key.publicBlob), /not a private key/)
  assert.throws(() => signManifest(key.privatePem, Buffer.alloc(0), key.publicBlob), /empty/)
  assert.throws(() => anchorKey(Buffer.alloc(411)), /RSA-3072 CNG public blob/)
  assert.throws(() => publicBlob(createPublicKey(small)), /RSA-3072/)
})

async function fixture() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'sign-release-'))
  const application = path.join(root, 'application')
  const crash = path.join(root, 'crash')
  await fs.mkdir(application)
  await fs.mkdir(crash)
  const names = ['sugarsubstitute.exe', 'sugarsubstitute.pdb', 'sugarsubstitute-application.json']
  for (const name of names) await fs.writeFile(path.join(application, name), name)
  await fs.writeFile(path.join(application, engineBuildFile), '{"schema_version":1,"release":"2","protocol":1}\n')
  await fs.writeFile(path.join(application, 'application-build.json'), JSON.stringify({ schema_version: 1, files: names.map(name => ({ name })) }))
  for (const name of crashHandlerFiles) await fs.writeFile(path.join(crash, name), name)
  const anchor = path.join(root, 'release-anchor.bin')
  await fs.writeFile(anchor, key.publicBlob)
  return { root, application, crashHandler: crash, anchor }
}

/** A manifest in the producer's layout: magic, schema, identity byte, then the key identity. */
function manifestFor(blob) {
  return Buffer.concat([Buffer.from('SSRELEASE-K1-v2\0'), Buffer.from([2, 0, 0, 0, 1]), createHash('sha256').update(blob).digest(), Buffer.from('rest')])
}

test('a signed release holds the build, the crash handler, the manifest and its signature', async () => {
  const { root, application, crashHandler, anchor } = await fixture()
  try {
    const output = path.join(root, 'signed')
    const calls = []
    const receipt = await signRelease({
      application, crashHandler, anchor, release: 'substitute-0_2_0', output, privatePem: key.privatePem,
      produceManifest: async request => { calls.push(request); return manifestFor(key.publicBlob) },
    })
    assert.deepEqual(calls, [{ root: output, release: 'substitute-0_2_0', anchor }])
    const names = (await fs.readdir(output)).sort()
    assert.deepEqual(names, [...crashHandlerFiles, engineBuildFile, manifestFile, signatureFile, 'release-signature.json',
      'sugarsubstitute-application.json', 'sugarsubstitute.exe', 'sugarsubstitute.pdb'].sort())
    const manifest = await fs.readFile(path.join(output, manifestFile))
    assert.ok(verifyManifest(manifest, await fs.readFile(path.join(output, signatureFile)), key.publicBlob))
    assert.equal(receipt.key_id, keyId(key.publicBlob))
    assert.equal(receipt.files.length, 8)
    await assert.rejects(signRelease({
      application, crashHandler, anchor, release: 'substitute-0_2_0', output, privatePem: key.privatePem,
      produceManifest: async () => manifestFor(key.publicBlob),
    }), /EEXIST/)
  } finally { await fs.rm(root, { recursive: true, force: true }) }
})

test('signing stops on a missing key, a foreign manifest key or a wrong private key', async () => {
  const { root, application, crashHandler, anchor } = await fixture()
  try {
    const base = { application, crashHandler, anchor, release: 'substitute-0_2_0', produceManifest: async () => manifestFor(key.publicBlob) }
    await assert.rejects(signRelease({ ...base, output: path.join(root, 'a') }), /RELEASE_MANIFEST_KEY/)
    await assert.rejects(signRelease({ ...base, output: path.join(root, 'b'), privatePem: key.privatePem,
      produceManifest: async () => manifestFor(generateReleaseKey().publicBlob) }), /names another key/)
    await assert.rejects(signRelease({ ...base, output: path.join(root, 'c'), privatePem: generateReleaseKey().privatePem }), /does not match/)
    await assert.rejects(signRelease({ ...base, output: path.join(root, 'd'), privatePem: key.privatePem, release: 'substitute-0.2.0' }), /release identity/)
    for (const name of ['b', 'c']) {
      await assert.rejects(fs.stat(path.join(root, name, signatureFile)), /ENOENT/)
    }
  } finally { await fs.rm(root, { recursive: true, force: true }) }
})

test('a rotated key is written once: the private PEM and the anchor it names', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'release-key-'))
  try {
    const privateFile = path.join(root, 'release-manifest.key.pem')
    const anchorFile = path.join(root, 'release-anchor.bin')
    const id = writeReleaseKey(privateFile, anchorFile)
    const anchor = await fs.readFile(anchorFile)
    assert.equal(id, keyId(anchor))
    assert.deepEqual(publicBlob(await fs.readFile(privateFile, 'utf8')), anchor)
    assert.throws(() => writeReleaseKey(privateFile, path.join(root, 'other.bin')), /EEXIST/)
    assert.throws(() => writeReleaseKey('relative.pem', anchorFile), /absolute/)
  } finally { await fs.rm(root, { recursive: true, force: true }) }
})