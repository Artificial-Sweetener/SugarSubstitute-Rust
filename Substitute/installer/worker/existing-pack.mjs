// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Refuse, in plain words, a SugarCubes already in the selected ComfyUI that predates this application. */
import fs from 'node:fs/promises'
import path from 'node:path'

/**
 * What every SugarCubes this application works with carries: the model-root owner setup
 * configures, and the release-source description the application keeps it up to date from.
 */
const required = [
  'load_model_roots.py',
  path.join('sugarcubes', 'backend', 'self_update', 'release_source.json'),
]

/** The largest pack metadata read for the version named in the refusal. */
const maxMetadataBytes = 65536

/** An installed SugarCubes older than this application works with. Setup changed nothing. */
export class OutdatedPackError extends Error {}

/** The installed pack as the refusal names it: with its version when the metadata states one. */
async function described(folder) {
  try {
    const handle = await fs.open(path.join(folder, 'pyproject.toml'))
    try {
      const { bytesRead, buffer } = await handle.read(Buffer.alloc(maxMetadataBytes), 0, maxMetadataBytes, 0)
      const version = /^version\s*=\s*"([0-9][0-9A-Za-z.+-]{0,31})"/m.exec(buffer.toString('utf8', 0, bytesRead))
      return version ? `SugarCubes ${version[1]}` : 'The SugarCubes'
    } finally {
      await handle.close()
    }
  } catch {
    return 'The SugarCubes'
  }
}

/**
 * Accept the SugarCubes in `folder`, or refuse it as older than this application works with.
 * Setup never replaces a pack it did not install: the user updates it or removes it.
 */
export async function requireSupportedPack(folder) {
  for (const file of required) {
    try {
      await fs.access(path.join(folder, file))
    } catch (cause) {
      if (cause.code !== 'ENOENT') throw cause
      throw new OutdatedPackError(
        `${await described(folder)} in this ComfyUI is older than SugarSubstitute works with. `
        + 'Update SugarCubes in that ComfyUI, or remove its SugarCubes folder from custom_nodes '
        + 'so setup installs the current version. Then run setup again.',
        { cause },
      )
    }
  }
}
