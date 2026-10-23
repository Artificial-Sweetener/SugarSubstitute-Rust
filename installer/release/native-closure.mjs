// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** One native dependency owner serves application and independent updater packaging. */
import path from 'node:path'
import { libraryName, isSystemLibrary } from './windows-imports.mjs'
import { artifactDirectory } from './artifact-files.mjs'

/** Resolve named and ordinal exports through forwarding chains; module import cycles are valid,
 * but a cycle of forwarded entry points never resolves to executable code.
 */
function requireEntryPoints(images) {
  const libraries = new Map(images.map(image => [image.name, image]))
  for (const image of images) {
    for (const imported of image.imports) {
      for (const entry of [...imported.symbols.map(name => ({ name })), ...imported.ordinals.map(ordinal => ({ ordinal }))]) {
        let library = imported.library, requested = entry
        const visited = new Set()
        while (!isSystemLibrary(library)) {
          const target = libraries.get(library)?.exports.find(value => requested.name !== undefined ? value.name === requested.name : value.ordinal === requested.ordinal)
          if (!target) throw new Error(`Native dependency does not provide the required entry points: ${library}`)
          const identity = `${library}:${target.ordinal}`
          if (visited.has(identity) || visited.size >= 4096) throw new Error('Forwarded native entry point does not resolve')
          visited.add(identity)
          if (!target.forward) break
          library = target.forward.library
          requested = target.forward.symbols.length ? { name: target.forward.symbols[0] } : { ordinal: target.forward.ordinals[0] }
        }
      }
    }
  }
}

/** Check copied native roots and copy only their explicitly supplied missing dependencies.
 * Each root is an executable or DLL owned by the calling component. Existing sibling images
 * satisfy imports without copying another library. Nothing resolves through the build host's PATH.
 * The caller's copy operation records each new file in its package inventory.
 */
export async function completeNativeClosure(directory, roots, runtimeFiles, inspect, copy) {
  await artifactDirectory(directory)
  if (!roots.length || roots.length > 128 || runtimeFiles.length > 127 ||
      roots.some(name => !/^[a-z0-9_][a-z0-9_.-]*\.(exe|dll)$/.test(name)) || new Set(roots).size !== roots.length) throw new Error('Invalid native dependency roots')
  const libraries = new Map()
  for (const source of runtimeFiles) {
    if (!path.isAbsolute(source)) throw new Error('Runtime inputs must be explicit absolute paths')
    const name = libraryName(path.basename(source))
    if (isSystemLibrary(name) || libraries.has(name) || roots.includes(name)) throw new Error('System or duplicate runtime library supplied')
    libraries.set(name, source)
  }
  const visited = new Set(), pending = [...roots], dependencies = []
  while (pending.length) {
    const name = pending.shift()
    if (visited.has(name)) continue
    visited.add(name)
    if (visited.size > 128) throw new Error('Native dependency closure exceeds its bound')
    const image = await inspect(path.join(directory, name))
    dependencies.push({ name, ...image })
    for (const imported of image.imports) {
      const dependency = libraryName(imported.library)
      if (isSystemLibrary(dependency) || visited.has(dependency) || pending.includes(dependency)) continue
      const source = libraries.get(dependency)
      if (!source) throw new Error(`Native dependency is missing: ${dependency}`)
      await copy(dependency, source)
      pending.push(dependency)
    }
  }
  if ([...libraries.keys()].some(name => !visited.has(name))) throw new Error('Unused runtime library supplied')
  requireEntryPoints(dependencies)
  return dependencies
}
