// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Assemble producing-installer configuration from local release-authorized metadata files. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { pathToFileURL } from 'node:url'

const roleLimit = 1024 ** 2
const configurationLimit = 4 * roleLimit

/** Preserve exact UTF-8 bytes, rejecting non-files, growth and oversized inputs before parsing. */
async function read(file, limit) {
  if (typeof file !== 'string' || !path.isAbsolute(file)) throw new Error('Distribution inputs require absolute file paths')
  const handle = await fs.open(file, 'r')
  try {
    const stat = await handle.stat()
    if (!stat.isFile() || stat.size === 0 || stat.size > limit) throw new Error('Distribution input exceeds its file bound')
    const chunks = []
    let length = 0
    for await (const chunk of handle.createReadStream({ autoClose: false, highWaterMark: 65536 })) {
      length += chunk.length
      if (length > limit) throw new Error('Distribution input grew beyond its bound')
      chunks.push(chunk)
    }
    if (length !== stat.size) throw new Error('Distribution input size changed')
    return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(Buffer.concat(chunks))
  } finally {
    await handle.close()
  }
}

/** This assembles bytes; the native importer authenticates them. No key is generated or used. */
export async function writeInstallerDistribution(input, output) {
  if (input === null || typeof input !== 'object' || Array.isArray(input) ||
      Object.keys(input).some(key => !['repository', 'channel', 'manifestSha256', 'trustedRoot', 'roots', 'timestamp', 'snapshot', 'targets', 'descriptor'].includes(key)) ||
      typeof input.repository !== 'string' || !/^[a-f0-9]{64}$/.test(input.repository) || /^0+$/.test(input.repository) ||
      typeof input.manifestSha256 !== 'string' || !/^[a-f0-9]{64}$/.test(input.manifestSha256) || !['release', 'canary'].includes(input.channel) ||
      !Array.isArray(input.roots) || input.roots.length > 1024 || !path.isAbsolute(output ?? '')) {
    throw new Error('Invalid installer distribution build inputs')
  }
  let previous = 0
  for (const root of input.roots) {
    if (root === null || typeof root !== 'object' || Array.isArray(root) ||
        Object.keys(root).some(key => !['version', 'file'].includes(key)) ||
        !Number.isInteger(root.version) || root.version <= previous || root.version > 0xffffffff) {
      throw new Error('Installer roots must have increasing positive versions')
    }
    previous = root.version
  }
  let retainedBytes = 0
  const readRole = async (file, limit) => {
    const text = await read(file, Math.min(limit, configurationLimit - retainedBytes))
    retainedBytes += Buffer.byteLength(JSON.stringify(text))
    if (retainedBytes > configurationLimit) throw new Error('Installer distribution configuration exceeds its encoded bound')
    return text
  }
  const configuration = {
    schema_version: 1, repository: [...Buffer.from(input.repository, 'hex')],
    channel: input.channel, manifest_sha256: input.manifestSha256,
    trusted_root: await readRole(input.trustedRoot, roleLimit), roots: [],
    timestamp: await readRole(input.timestamp, roleLimit), snapshot: await readRole(input.snapshot, roleLimit),
    targets: await readRole(input.targets, roleLimit), descriptor: await readRole(input.descriptor, 8192),
  }
  for (const root of input.roots) configuration.roots.push({ version: root.version, metadata: await readRole(root.file, roleLimit) })
  const bytes = Buffer.from(JSON.stringify(configuration))
  if (bytes.length > configurationLimit) throw new Error('Installer distribution configuration exceeds its encoded bound')
  await fs.writeFile(output, bytes, { flag: 'wx' })
  return { configuration: output, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [source, output, ...extra] = process.argv.slice(2)
  if (!source || !output || extra.length) throw new Error('Usage: node installer-distribution.mjs <absolute-input-json> <new-absolute-output>')
  console.log(JSON.stringify(await writeInstallerDistribution(JSON.parse(await read(source, 256 * 1024)), output)))
}
