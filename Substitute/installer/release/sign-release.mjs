// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Join one release build of the application with its crash handler and sign their release
 * manifest, producing the application directory `installer/package.mjs` assembles from.
 *
 * The manifest binds the actual host, crash client and crash handler images and symbols; the
 * signature is RSA-PSS under the key the committed release anchor names. The private key
 * arrives only through the RELEASE_MANIFEST_KEY environment variable.
 */
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { copyArtifact, newArtifactDirectory, regularArtifact } from './artifact-files.mjs'
import { engineBuildFile } from './engine-identity.mjs'
import { keyId, signManifest } from './release-signature.mjs'

export const crashHandlerFiles = [
  'crashpad_handler.exe', 'crashpad_handler.pdb',
  'sugarsubstitute_crashpad_client.dll', 'sugarsubstitute_crashpad_client.pdb',
]
export const manifestFile = 'sugarsubstitute-release-v1.manifest'
export const signatureFile = 'sugarsubstitute-release-v1.sig'
const hostImage = 'sugarsubstitute.exe'
const manifestProducer = fileURLToPath(new URL('../../tools/local_k1/build_manifest.py', import.meta.url))
/** Manifest header: 16-byte magic, u32 schema, u8 identity, then the 32-byte key identity. */
const keyIdOffset = 21

/**
 * Run the repository's PE/PDB manifest producer over the assembled directory. Python ignores its
 * environment and user site packages but keeps the producer's own directory, which holds the
 * manifest codec it imports.
 */
export function pythonManifestProducer(python) {
  return async ({ root, release, anchor }) => {
    const scratch = await fs.mkdtemp(path.join(os.tmpdir(), 'release-manifest-'))
    try {
      const result = spawnSync(python, ['-E', '-s', manifestProducer, '--release-root', root, '--host-image', hostImage,
        '--release-build', release, '--public-anchor', anchor, '--output-root', scratch],
      { encoding: 'utf8', windowsHide: true, env: { ...process.env, PYTHONDONTWRITEBYTECODE: '1' } })
      if (result.error) throw result.error
      if (result.status !== 0) throw new Error(`Release manifest production failed: ${result.stderr.trim()}`)
      return await fs.readFile(path.join(scratch, manifestFile))
    } finally {
      await fs.rm(scratch, { recursive: true, force: true })
    }
  }
}

/**
 * Copy the application build's recorded outputs and the crash handler's four images and symbols
 * into a new `output`, then write the manifest and its signature. Nothing is replaced.
 */
export async function signRelease({ application, crashHandler, anchor, release, output, privatePem, produceManifest }) {
  for (const value of [application, crashHandler, anchor, output]) {
    if (!value || !path.isAbsolute(value)) throw new Error('All release paths must be absolute')
  }
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(release)) throw new Error('Invalid release identity')
  if (!privatePem) throw new Error('Set RELEASE_MANIFEST_KEY to the release manifest private key')
  if ((await regularArtifact(anchor)).size !== 411) throw new Error('The release anchor is not a 411-byte public blob')
  const anchorBytes = await fs.readFile(anchor)
  const build = JSON.parse(await fs.readFile(path.join(application, 'application-build.json'), 'utf8'))
  if (build.schema_version !== 1 || !Array.isArray(build.files)) throw new Error('Not an application release build')
  const names = [engineBuildFile, ...build.files.map(file => file.name)]
  if (!names.includes(hostImage)) throw new Error('The application build has no host image')
  await newArtifactDirectory(output)
  const files = []
  for (const name of names) files.push({ name, ...await copyArtifact(path.join(application, name), path.join(output, name)) })
  for (const name of crashHandlerFiles) files.push({ name, ...await copyArtifact(path.join(crashHandler, name), path.join(output, name)) })

  const manifest = await produceManifest({ root: output, release, anchor })
  if (!manifest.subarray(keyIdOffset, keyIdOffset + 32).equals(Buffer.from(keyId(anchorBytes), 'hex'))) {
    throw new Error('The produced manifest names another key than the release anchor')
  }
  const signature = signManifest(privatePem, manifest, anchorBytes)
  await fs.writeFile(path.join(output, manifestFile), manifest, { flag: 'wx' })
  await fs.writeFile(path.join(output, signatureFile), signature, { flag: 'wx' })
  const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')
  const receipt = {
    schema_version: 1, release, key_id: keyId(anchorBytes),
    manifest_sha256: sha256(manifest), signature_sha256: sha256(signature), files,
  }
  await fs.writeFile(path.join(output, 'release-signature.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' })
  return receipt
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [application, crashHandler, anchor, release, python, output, ...extra] = process.argv.slice(2)
  if (!output || extra.length || !path.isAbsolute(python ?? '')) {
    throw new Error('Usage: node installer/release/sign-release.mjs <application-build> <crash-handler-build> <release-anchor> <release-id> <absolute-python> <new-output>; RELEASE_MANIFEST_KEY holds the private key')
  }
  const receipt = await signRelease({
    application, crashHandler, anchor, release, output,
    privatePem: process.env.RELEASE_MANIFEST_KEY, produceManifest: pythonManifestProducer(python),
  })
  console.log(JSON.stringify({ output, ...receipt }))
}
