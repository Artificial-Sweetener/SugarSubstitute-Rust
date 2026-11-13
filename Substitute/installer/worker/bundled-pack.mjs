// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Read the bundled SugarCubes registry archive before setup installs it as a registry install. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { execFile } from 'node:child_process'

/** Where the application package carries SugarCubes, relative to the package root. */
export const bundledArchive = path.join('sugarcubes', 'SugarCubes.zip')

/**
 * Extract the bundled archive into a fresh folder under `parent` with the worker's own 7-Zip, so
 * dependency inspection and the model-root owner read the exact pack setup is about to install.
 * Native setup storage remains the only owner that installs it. Returns the pack folder and a
 * disposer that removes the folder; a failed extraction removes it before rejecting.
 */
export async function stageBundledPack(archive, parent, sevenZip, signal) {
  signal.throwIfAborted()
  await fs.access(archive)
  await fs.mkdir(parent, { recursive: true })
  const folder = await fs.mkdtemp(path.join(parent, 'sugarcubes-'))
  const dispose = () => fs.rm(folder, { recursive: true, force: true })
  try {
    await new Promise((resolve, reject) => {
      execFile(sevenZip, ['x', '-y', '-bd', `-o${folder}`, archive], { signal, windowsHide: true, maxBuffer: 4 * 1024 * 1024 },
        (cause, _stdout, stderr) => cause ? reject(new Error(`The bundled SugarCubes archive could not be read: ${String(stderr).slice(-1024)}`, { cause })) : resolve())
    })
    await fs.access(path.join(folder, 'pyproject.toml'))
    return { folder, dispose }
  } catch (cause) {
    await dispose()
    throw cause
  }
}