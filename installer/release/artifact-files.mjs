// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Bounded regular release inputs and exclusive, hashed artifact copies. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'

/** Verify each existing component without resolving a link to another ownership root. */
async function unlinked(file) {
  if (!path.isAbsolute(file)) throw new Error('Artifact paths must be absolute')
  let current = file
  while (true) {
    const info = await fs.lstat(current)
    if (info.isSymbolicLink()) throw new Error('Artifacts must not traverse links')
    const parent = path.dirname(current)
    if (parent === current) break
    current = parent
  }
  return fs.lstat(file)
}

/** Reject linked ancestry as well as a linked leaf before consuming build inputs. */
export async function regularArtifact(file) {
  const info = await unlinked(file)
  if (!info.isFile() || info.size > 2 * 1024 ** 3) throw new Error('Artifact is not a bounded regular file')
  return info
}

/** Output parents must be actual directories before a build can create or copy anything. */
export async function artifactDirectory(directory) {
  if (!(await unlinked(directory)).isDirectory()) throw new Error('Artifact parent is not a directory')
}

/** Create only a fresh child of verified existing ancestry, preserving every existing output. */
export async function newArtifactDirectory(directory) {
  if (!path.isAbsolute(directory)) throw new Error('Artifact destination must be absolute')
  await artifactDirectory(path.dirname(directory))
  await fs.mkdir(directory)
}

/** Copy into a new path, detect changed input identity and hash the actual delivered bytes. */
export async function copyArtifact(source, destination) {
  const before = await regularArtifact(source)
  await artifactDirectory(path.dirname(destination))
  await fs.copyFile(source, destination, fs.constants.COPYFILE_EXCL)
  const hash = createHash('sha256')
  for await (const chunk of createReadStream(destination)) hash.update(chunk)
  const after = await regularArtifact(source)
  const copied = await regularArtifact(destination)
  if (before.size !== after.size || before.mtimeMs !== after.mtimeMs || before.ino !== after.ino || copied.size !== before.size) {
    throw new Error('Artifact changed during assembly')
  }
  return { bytes: copied.size, sha256: hash.digest('hex') }
}
