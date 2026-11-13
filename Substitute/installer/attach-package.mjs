// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Attach an already-authenticated bare archive without changing its signed package identity. */
import fs from 'node:fs/promises'
import { createWriteStream } from 'node:fs'
import { pipeline } from 'node:stream/promises'
import { createHash } from 'node:crypto'
import path from 'node:path'

/** The expected digest comes from release metadata; the executable is the producing setup build. */
export async function attachPackage(executable, archive, expected, output) {
  if ([executable, archive, output].some(value => !value || !path.isAbsolute(value)) ||
      !/^[a-f0-9]{64}$/.test(expected ?? '')) {
    throw new Error('Attachment requires absolute paths and the signed package SHA-256')
  }
  const source = await fs.open(archive, 'r')
  try {
    const metadata = await source.stat()
    if (!metadata.isFile() || metadata.size < 24 || metadata.size > 8 * 1024 ** 3) {
      throw new Error('Invalid bare package size')
    }
    const footer = Buffer.alloc(24)
    const tail = await source.read(footer, 0, footer.length, metadata.size - footer.length)
    if (tail.bytesRead !== footer.length || footer.subarray(16).toString() !== 'SSSETUP1') {
      throw new Error('Bare package footer is invalid')
    }
    const compressed = footer.readBigUInt64LE(0)
    const inventory = footer.readBigUInt64LE(8)
    if (inventory > 1024n ** 2n || compressed + inventory + 24n !== BigInt(metadata.size)) {
      throw new Error('Package must be a bounded bare archive with no executable envelope')
    }
    const packageHash = createHash('sha256')
    const installerHash = createHash('sha256')
    const manifestHash = createHash('sha256')
    const streamedFooter = Buffer.alloc(24)
    let packageBytes = 0
    let installerBytes = 0
    async function* contents() {
      const stub = await fs.open(executable, 'r')
      try {
        const metadata = await stub.stat()
        if (!metadata.isFile() || metadata.size === 0) throw new Error('Installer stub must be a nonempty regular file')
        for await (const chunk of stub.createReadStream({ autoClose: false })) {
          installerHash.update(chunk)
          installerBytes += chunk.length
          yield chunk
        }
      } finally {
        await stub.close()
      }
      for await (const chunk of source.createReadStream({ autoClose: false, start: 0 })) {
        const offset = packageBytes
        packageBytes += chunk.length
        if (packageBytes > metadata.size) throw new Error('Package size changed during attachment')
        const manifestStart = Math.max(offset, Number(compressed))
        const manifestEnd = Math.min(packageBytes, metadata.size - 24)
        if (manifestStart < manifestEnd) manifestHash.update(chunk.subarray(manifestStart - offset, manifestEnd - offset))
        const footerStart = Math.max(offset, metadata.size - 24)
        if (footerStart < packageBytes) chunk.copy(streamedFooter, footerStart - (metadata.size - 24), footerStart - offset)
        packageHash.update(chunk)
        installerHash.update(chunk)
        installerBytes += chunk.length
        yield chunk
      }
      if (packageBytes !== metadata.size || packageHash.digest('hex') !== expected || !streamedFooter.equals(footer)) {
        throw new Error('Package authentication failed during attachment')
      }
    }
    // Exclusive creation retains failed output for inspection; only success returns a sealed receipt.
    await pipeline(contents(), createWriteStream(output, { flags: 'wx' }))
    return {
      executable: output, bytes: installerBytes, sha256: installerHash.digest('hex'),
      package: archive, packageBytes, packageSha256: expected,
      manifestSha256: manifestHash.digest('hex'),
    }
  } finally {
    await source.close()
  }
}
