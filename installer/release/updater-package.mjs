// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Assemble one independently deployable updater with its complete reviewed native dependency closure. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { copyArtifact, newArtifactDirectory } from './artifact-files.mjs'
import { completeNativeClosure } from './native-closure.mjs'
import { writeInventory } from '../package-inventory.mjs'

const roles = ['sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb', 'sugarsubstitute-updater.json']

/** Only Cargo-selected engine roles and explicitly supplied libraries can enter the package.
 * The inspector reads copied bytes. Missing, unused, aliased or wrong-architecture inputs fail
 * before a completed manifest exists. Failure preserves the incomplete output for diagnosis.
 */
export async function packageUpdater(selection, runtimeFiles, output, release, inspect) {
  if (!path.isAbsolute(output) || !/^[a-zA-Z0-9_-]{1,64}$/.test(release)) throw new Error('Invalid updater package destination or release identity')
  if (selection.component !== 'updater' || selection.files.size !== roles.length || roles.some(name => !selection.files.has(name))) throw new Error('Updater requires exactly its producing executable, symbols and contract')
  await newArtifactDirectory(output)
  await fs.mkdir(path.join(output, 'app'))
  const files = []
  const copy = async (name, source) => {
    const destination = path.join(output, 'app', name)
    files.push({ path: `app/${name}`, ...await copyArtifact(source, destination) })
    return destination
  }
  for (const name of roles) await copy(name, selection.files.get(name))
  const dependencies = await completeNativeClosure(path.join(output, 'app'), ['sugarsubstitute-updater.exe'], runtimeFiles, inspect, copy)
  const manifestSha256 = await writeInventory(output, release, files)
  return { manifestSha256, dependencies, files }
}
