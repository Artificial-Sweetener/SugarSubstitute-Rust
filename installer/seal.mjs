// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Seal the shared native payload as an installer executable or a bare update archive. */
import fs from 'node:fs/promises'
import { createReadStream, createWriteStream } from 'node:fs'
import { pipeline } from 'node:stream/promises'
import { createGzip } from 'node:zlib'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { attachPackage } from './attach-package.mjs'

const arguments_ = process.argv.slice(2)
if (arguments_[0] === '--attach') {
  if (arguments_.length !== 5) throw new Error('Usage: node installer/seal.mjs --attach <compiled-setup-exe> <signed-package> <package-sha256> <new-output>')
  console.log(JSON.stringify(await attachPackage(...arguments_.slice(1))))
} else {
  await seal(arguments_)
}

/** Build one bare archive or an installer carrying a newly encoded archive. */
async function seal(arguments_) {
  const [executable, bundle, output] = arguments_
  const update = executable === '--update'
  if (arguments_.length !== 3 || [bundle, output, ...(update ? [] : [executable])].some(value => !value || !path.isAbsolute(value))) {
    throw new Error('Usage: node installer/seal.mjs <compiled-setup-exe|--update> <bundle> <new-output>')
  }
  const manifest = await fs.readFile(path.join(bundle, 'setup-manifest.json'))
  const inventory = JSON.parse(manifest)
  if (inventory.schema_version !== 1 || !Array.isArray(inventory.files) || !inventory.files.length || inventory.files.length > 4096 || manifest.length > 1024 ** 2) {
    throw new Error('Invalid installer inventory')
  }
  const names = new Set()
  let expanded = 0
  async function* payloads() {
    for (const file of inventory.files) {
      if (!Number.isSafeInteger(file.bytes) || file.bytes < 0 || file.bytes > 2 * 1024 ** 3) throw new Error('Invalid expanded file size')
      expanded += file.bytes
      if (expanded > 16 * 1024 ** 3) throw new Error('Expanded package exceeds its native bound')
      if (!/^(app|worker|electron|sugarcubes)\//.test(file.path) ||
          file.path.includes('\\') || file.path.split('/').some(part => !part || part === '..' || part === '.') ||
          names.has(file.path.toLowerCase())) throw new Error('Unsafe archive inventory')
      names.add(file.path.toLowerCase())
      const source = path.join(bundle, file.path)
      const metadata = await fs.lstat(source)
      if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== file.bytes) throw new Error('Payload changed')
      const digest = createHash('sha256')
      for await (const chunk of createReadStream(source)) { digest.update(chunk); yield chunk }
      if (digest.digest('hex') !== file.sha256) throw new Error('Payload authentication failed')
    }
  }
  const compressed = `${output}.payload.gz`
  await pipeline(payloads(), createGzip({ level: 6 }), createWriteStream(compressed, { flags: 'wx' }))
  if (!update) await fs.copyFile(executable, output, fs.constants.COPYFILE_EXCL)
  await pipeline(createReadStream(compressed), createWriteStream(output, { flags: update ? 'wx' : 'a' }))
  const footer = Buffer.alloc(24)
  footer.writeBigUInt64LE(BigInt((await fs.stat(compressed)).size), 0)
  footer.writeBigUInt64LE(BigInt(manifest.length), 8)
  footer.write('SSSETUP1', 16)
  await fs.appendFile(output, Buffer.concat([manifest, footer]))
  await fs.unlink(compressed)
  const archiveHash = createHash('sha256')
  for await (const chunk of createReadStream(output)) archiveHash.update(chunk)
  console.log(JSON.stringify({
    ...(update ? { package: output } : { executable: output }), bytes: (await fs.stat(output)).size,
    sha256: archiveHash.digest('hex'),
    manifestSha256: createHash('sha256').update(manifest).digest('hex'),
  }))
}
