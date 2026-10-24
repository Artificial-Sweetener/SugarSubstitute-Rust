// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Assemble the host, launcher and updater outputs reported by the same successful Cargo invocation. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { copyArtifact, newArtifactDirectory, regularArtifact } from './artifact-files.mjs'
import { engineBuildFile, readEngineIdentity } from './engine-identity.mjs'

const targets = {
  application: ['sugarsubstitute', 'sugarsubstitute-launcher', 'sugarsubstitute-updater'],
  updater: ['sugarsubstitute-updater'],
  setup: ['sugarsubstitute-setup'],
}

/** One component registry owns both Cargo invocation and output admission. */
export function nativeTargets(component) {
  if (!Object.hasOwn(targets, component)) throw new Error('Unknown native build component')
  return [...targets[component]]
}

function beneath(root, candidate) {
  const relative = path.relative(root, candidate)
  return path.isAbsolute(candidate) && relative !== '' && !relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative)
}

/** Require one successful build, exact package identity and all independently named targets. */
export function selectArtifacts(messages, workspace, target, component) {
  const names = nativeTargets(component)
  const manifest = path.join(workspace, 'crates', 'sugarsubstitute', 'Cargo.toml')
  const artifacts = messages.filter(message => message.reason === 'compiler-artifact' && message.manifest_path === manifest && message.target?.kind?.includes('bin') && names.includes(message.target.name))
  const finished = messages.filter(message => message.reason === 'build-finished')
  if (finished.length !== 1 || finished[0].success !== true || artifacts.length !== names.length) throw new Error('Cargo did not produce one successful complete component build')
  const packageId = artifacts[0].package_id
  if (!packageId || artifacts.some(artifact => artifact.package_id !== packageId)) throw new Error('Application targets belong to different packages')
  const scripts = messages.filter(message => message.reason === 'build-script-executed' && message.package_id === packageId)
  if (scripts.length !== 1 || !beneath(target, scripts[0].out_dir)) throw new Error('Producing application contract directory is absent or ambiguous')
  const files = new Map()
  for (const name of names) {
    const matches = artifacts.filter(artifact => artifact.target.name === name)
    if (matches.length !== 1) throw new Error('Duplicate or absent Cargo executable target')
    const artifact = matches[0]
    for (const extension of ['.exe', '.pdb']) {
      const leaf = name.replaceAll('-', '_') + extension
      const expected = extension === '.exe' ? `${name}.exe` : leaf
      const candidates = artifact.filenames.filter(file => path.basename(file) === expected && beneath(target, file))
      if (candidates.length !== 1 || (extension === '.exe' && artifact.executable !== candidates[0])) throw new Error('Cargo executable or symbols are absent or ambiguous')
      files.set(expected, candidates[0])
    }
  }
  const declarations = component === 'setup' ? ['installer-distribution.json'] : names.map(name => name === 'sugarsubstitute' ? 'sugarsubstitute-application.json' : `${name}.json`)
  if (component === 'application') declarations.push('sugarsubstitute-installation-activation.json')
  for (const name of declarations) {
    files.set(name, path.join(scripts[0].out_dir, name))
  }
  return { component, packageId, files, ...(component === 'setup' ? {} : { engineBuild: path.join(scripts[0].out_dir, engineBuildFile) }) }
}

/** Create a fresh artifact directory and record exact copied bytes without executing the app. */
export async function assembleArtifacts(selection, output) {
  if (!path.isAbsolute(output)) throw new Error('Artifact destination must be absolute')
  if (!Object.hasOwn(targets, selection.component)) throw new Error('Unknown artifact component')
  if (selection.component === 'setup') {
    const distribution = await regularArtifact(selection.files.get('installer-distribution.json'))
    if (distribution.size === 0 || distribution.size > 4 * 1024 ** 2) throw new Error('Producing installer distribution is absent or oversized')
  } else {
    await readEngineIdentity(selection.engineBuild)
  }
  await newArtifactDirectory(output)
  let engineMetadata = {}
  if (selection.component !== 'setup') {
    const engineBuild = { name: engineBuildFile, ...await copyArtifact(selection.engineBuild, path.join(output, engineBuildFile)) }
    const engine = await readEngineIdentity(path.join(output, engineBuildFile))
    engineMetadata = { engine, engine_build: engineBuild }
  }
  const files = []
  for (const [name, source] of selection.files) {
    const destination = path.join(output, name)
    files.push({ name, ...await copyArtifact(source, destination) })
  }
  const receipt = { schema_version: 1, package_id: selection.packageId, ...engineMetadata, files,
    ...(selection.configuration ? { configuration: selection.configuration, bootstrap: selection.bootstrap } : {}),
    scope: 'Cargo build outputs; release signing and package authentication are separate operations' }
  await fs.writeFile(path.join(output, `${selection.component}-build.json`), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' })
  return receipt
}
