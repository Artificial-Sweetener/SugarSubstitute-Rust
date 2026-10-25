// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Shared producing-side bounds and exact native package inventory encoding. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { artifactDirectory } from './release/artifact-files.mjs'

/** Mirror the public package-path contract, including Windows aliases on other build hosts. */
function packagePath(value) {
  if (typeof value !== 'string' || Buffer.byteLength(value) > 1024 || !/^(app|worker|sugarcubes)\//.test(value) || /[:\\\p{Cc}<>"|?*]/u.test(value)) return false
  return value.split('/').every(part => {
    const stem = part.split('.')[0].replace(/ +$/, '').toUpperCase()
    return part && part !== '.' && part !== '..' && part.length <= 255 && !/[. ]$/.test(part) &&
      !/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|(?:COM|LPT)[1-9¹²³])$/.test(stem)
  })
}

export async function writeInventory(output, release, files) {
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(release)) throw new Error('Invalid release identity')
  const ordered = [...files].sort((left, right) => left.path.localeCompare(right.path, 'en'))
  if (!ordered.length || ordered.length > 4096 || new Set(ordered.map(file => file.path.toLowerCase())).size !== ordered.length) {
    throw new Error('Package inventory is ambiguous or exceeds its native bound')
  }
  let expanded = 0
  for (const file of ordered) {
    if (!packagePath(file.path) ||
        !Number.isSafeInteger(file.bytes) || file.bytes < 0 || file.bytes > 2 * 1024 ** 3 || !/^[a-f0-9]{64}$/.test(file.sha256)) throw new Error('Invalid package entry')
    expanded += file.bytes
  }
  if (expanded > 16 * 1024 ** 3) throw new Error('Expanded package exceeds its native bound')
  const manifest = Buffer.from(JSON.stringify({ schema_version: 1, release_id: release, files: ordered }))
  if (manifest.length > 1024 ** 2) throw new Error('Package manifest exceeds its native bound')
  await artifactDirectory(output)
  await fs.writeFile(path.join(output, 'setup-manifest.json'), manifest, { flag: 'wx' })
  return createHash('sha256').update(manifest).digest('hex')
}
