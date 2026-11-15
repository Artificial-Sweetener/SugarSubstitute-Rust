// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Write an installer's distribution from the release the update repository publishes. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { pathToFileURL } from 'node:url'
import { writeInstallerDistribution } from './installer-distribution.mjs'
import { releaseDistribution, repositoryRoot } from './update-distribution.mjs'

const metadataLimit = 1024 ** 2
const descriptorLimit = 8192
const packageLimit = 8 * 1024 ** 3
const footerBytes = 24
const platforms = ['windows-x86_64', 'windows-aarch64']
const plainVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/

/** One JSON file of the repository folder, with its path. */
async function published(repository, name, limit) {
  const file = path.join(repository, name)
  const info = await fs.stat(file).catch(() => null)
  if (!info?.isFile() || info.size === 0 || info.size > limit) throw new Error(`The update repository has no usable ${name}`)
  return { file, value: JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await fs.readFile(file))) }
}

function metadataVersion(value, what) {
  if (!Number.isInteger(value) || value < 1 || value > 0xffffffff) throw new Error(`${what} names no metadata version`)
  return value
}

/**
 * The files of a downloaded update repository that describe the application release it
 * publishes for `platform` on the release channel, found as an installation finds them: the
 * timestamp names the snapshot, the snapshot names the targets, and the channel descriptor
 * names the package. Nothing is authenticated here. The installer authenticates every one of
 * these files against its own trust when it installs.
 */
export async function publishedRelease(repository, platform) {
  if (!path.isAbsolute(repository ?? '') || !platforms.includes(platform)) throw new Error('An absolute repository folder and a Windows platform are required')
  const timestamp = await published(repository, 'timestamp.json', metadataLimit)
  const snapshotVersion = metadataVersion(timestamp.value?.signed?.meta?.['snapshot.json']?.version, 'timestamp.json')
  const snapshot = await published(repository, `${snapshotVersion}.snapshot.json`, metadataLimit)
  const targetsVersion = metadataVersion(snapshot.value?.signed?.meta?.['targets.json']?.version, snapshot.file)
  const targets = await published(repository, `${targetsVersion}.targets.json`, metadataLimit)
  const name = `application-release-${platform}.json`
  if (!Object.hasOwn(targets.value?.signed?.targets ?? {}, name)) throw new Error(`The update repository publishes no application release for ${platform}`)
  const descriptor = await published(repository, name, descriptorLimit)
  const offer = descriptor.value
  if (offer?.product !== 'sugarsubstitute' || offer.channel !== 'release' || offer.platform !== platform || offer.component !== 'application' ||
      !plainVersion.test(offer.version ?? '') || !/^[a-f0-9]{64}$/.test(offer.package_sha256 ?? '') ||
      !Number.isSafeInteger(offer.package_bytes) || offer.package_bytes < footerBytes || offer.package_bytes > packageLimit) {
    throw new Error(`${name} does not offer an application release for ${platform}`)
  }
  return {
    version: offer.version, timestamp: timestamp.file, snapshot: snapshot.file, targets: targets.file, descriptor: descriptor.file,
    package: { name: `${offer.package_sha256}.ssupdate`, sha256: offer.package_sha256, bytes: offer.package_bytes },
  }
}

/** Every root the repository publishes after the one installations ship with, oldest first. */
export async function newerRoots(repository, trustedRoot) {
  const shipped = metadataVersion(JSON.parse(await fs.readFile(trustedRoot, 'utf8'))?.signed?.version, 'The trusted root')
  const roots = []
  for (let version = shipped + 1; ; version += 1) {
    const file = path.join(repository, `${version}.root.json`)
    if (!(await fs.stat(file).catch(() => null))?.isFile()) return roots
    roots.push({ version, file })
  }
}

/** The length, digest and inventory digest of a sealed package, from its own bytes. */
export async function sealedPackage(file) {
  if (!path.isAbsolute(file ?? '')) throw new Error('The sealed package needs an absolute path')
  const handle = await fs.open(file, 'r')
  try {
    const { size } = await handle.stat()
    if (size < footerBytes || size > packageLimit) throw new Error('The sealed package has no footer or is oversized')
    const footer = Buffer.alloc(footerBytes)
    await handle.read(footer, 0, footerBytes, size - footerBytes)
    const compressed = footer.readBigUInt64LE(0)
    const inventory = footer.readBigUInt64LE(8)
    if (footer.subarray(16).toString() !== 'SSSETUP1' || inventory === 0n || inventory > 1024n ** 2n || compressed + inventory + BigInt(footerBytes) !== BigInt(size)) {
      throw new Error('The sealed package footer is invalid')
    }
    const manifest = Buffer.alloc(Number(inventory))
    await handle.read(manifest, 0, manifest.length, Number(compressed))
    const digest = createHash('sha256')
    let bytes = 0
    for await (const chunk of createReadStream(file)) { digest.update(chunk); bytes += chunk.length }
    if (bytes !== size) throw new Error('The sealed package changed while it was read')
    return { bytes, sha256: digest.digest('hex'), manifestSha256: createHash('sha256').update(manifest).digest('hex') }
  } finally {
    await handle.close()
  }
}

/** The repository identity release builds compile in, as the installer distribution spells it. */
async function repositoryIdentity(distribution) {
  const identity = JSON.parse(await fs.readFile(distribution, 'utf8'))?.repository
  if (!Array.isArray(identity) || identity.length !== 32 || identity.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
    throw new Error('The release distribution names no repository identity')
  }
  return Buffer.from(identity).toString('hex')
}

/**
 * Write the distribution an installer of `version` is compiled with: the repository's signed
 * metadata for that release and the inventory digest of its package. `packageFile` is the
 * package the descriptor offers, byte for byte. An installer can be built only for the release
 * the repository currently publishes, because only that release is described by its metadata.
 */
export async function writeReleaseInstallerDistribution({ repository, version, platform, packageFile, output, trustedRoot = repositoryRoot, distribution = releaseDistribution }) {
  const release = await publishedRelease(repository, platform)
  if (release.version !== version) throw new Error(`The update repository publishes ${release.version}, not ${version}; an installer is built for the published release`)
  const sealed = await sealedPackage(packageFile)
  if (sealed.sha256 !== release.package.sha256 || sealed.bytes !== release.package.bytes) throw new Error('The package is not the one the update repository offers')
  const receipt = await writeInstallerDistribution({
    repository: await repositoryIdentity(distribution), channel: 'release', manifestSha256: sealed.manifestSha256,
    trustedRoot, roots: await newerRoots(repository, trustedRoot),
    timestamp: release.timestamp, snapshot: release.snapshot, targets: release.targets, descriptor: release.descriptor,
  }, output)
  return { ...receipt, version, package: release.package.name, packageSha256: sealed.sha256, manifestSha256: sealed.manifestSha256 }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const usage = 'Usage: node installer/release/published-release.mjs package <repository-folder> <version> <platform> | distribution <repository-folder> <version> <platform> <package-file> <new-absolute-output>'
  const [command, folder, version, platform, packageFile, output, ...extra] = process.argv.slice(2)
  if (!folder || !plainVersion.test(version ?? '') || !platform) throw new Error(usage)
  const repository = path.resolve(folder)
  if (command === 'package' && !packageFile) {
    const release = await publishedRelease(repository, platform)
    if (release.version !== version) throw new Error(`The update repository publishes ${release.version}, not ${version}; an installer is built for the published release`)
    console.log(release.package.name)
  } else if (command === 'distribution' && output && !extra.length) {
    console.log(JSON.stringify(await writeReleaseInstallerDistribution({ repository, version, platform, packageFile: path.resolve(packageFile), output })))
  } else {
    throw new Error(usage)
  }
}
