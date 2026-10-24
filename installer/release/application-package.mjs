// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Assemble native application roles, their runtime closure and the existing provisioning payload. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { artifactDirectory, copyArtifact, newArtifactDirectory } from './artifact-files.mjs'
import { completeNativeClosure } from './native-closure.mjs'
import { requireBootstrapDependencies } from './windows-imports.mjs'
import { writeInventory } from '../package-inventory.mjs'

const nativeRoles = [
  'sugarsubstitute.exe', 'sugarsubstitute.pdb',
  'sugarsubstitute_crashpad_client.dll', 'sugarsubstitute_crashpad_client.pdb',
  'crashpad_handler.exe', 'crashpad_handler.pdb',
  'sugarsubstitute-release-v1.manifest', 'sugarsubstitute-release-v1.sig',
  'sugarsubstitute-application.json',
  'sugarsubstitute-updater.exe', 'sugarsubstitute_updater.pdb', 'sugarsubstitute-updater.json',
  'sugarsubstitute-launcher.exe', 'sugarsubstitute_launcher.pdb', 'sugarsubstitute-launcher.json',
  'sugarsubstitute-installation-activation.json',
]

/** Inspect actual copied host, launcher, updater and crash-handler images before publishing
 * an inventory. Explicit runtime inputs are copied beside the app images that load them.
 * Producing compatibility declarations and Comfy provisioning choices remain byte-for-byte.
 */
export async function packageApplication({ app, cubes, build, output, release, runtimeFiles }, inspect) {
  for (const value of [app, cubes, output, build]) {
    if (!value || !path.isAbsolute(value)) throw new Error('All package paths must be absolute')
  }
  if (!/^[a-zA-Z0-9_-]{1,64}$/.test(release)) throw new Error('Invalid release identity')
  await newArtifactDirectory(output)
  const files = []
  async function copyFile(source, relative) {
    const destination = path.join(output, relative)
    await fs.mkdir(path.dirname(destination), { recursive: true })
    files.push({ path: relative, ...await copyArtifact(source, destination) })
  }
  async function copyTree(source, relative) {
    const info = await fs.lstat(source)
    if (info.isSymbolicLink()) throw new Error(`Linked package source: ${relative}`)
    if (!info.isDirectory()) return copyFile(source, relative)
    await artifactDirectory(source)
    for (const leaf of (await fs.readdir(source)).sort()) {
      if (leaf === '__pycache__' || leaf.endsWith('.pyc')) continue
      await copyTree(path.join(source, leaf), `${relative}/${leaf}`)
    }
  }
  for (const leaf of nativeRoles) await copyFile(path.join(app, leaf), `app/${leaf}`)
  const dependencies = await completeNativeClosure(
    path.join(output, 'app'), nativeRoles.filter(name => /\.(exe|dll)$/.test(name)),
    runtimeFiles, inspect, (name, source) => copyFile(source, `app/${name}`),
  )
  requireBootstrapDependencies(dependencies.filter(image => ['sugarsubstitute.exe', 'sugarsubstitute-launcher.exe', 'sugarsubstitute-updater.exe'].includes(image.name)))
  for (const leaf of [
    '__init__.py', 'prestartup_script.py', 'load_model_roots.py', 'pyproject.toml',
    'requirements.txt', 'LICENSE', 'sugarcubes', 'sugarcubes_model_roots', 'web',
  ]) await copyTree(path.join(cubes, leaf), `sugarcubes/${leaf}`)
  await copyTree(path.join(build, 'worker'), 'worker')
  await copyTree(path.join(build, 'electron'), 'electron')
  const manifestSha256 = await writeInventory(output, release, files)
  return { manifestSha256, dependencies, files }
}
