// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Keep self-contained Windows distribution builds in an explicitly owned compiler configuration. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { artifactDirectory, newArtifactDirectory, regularArtifact } from './artifact-files.mjs'

const targets = { 'windows-x86_64': 'x86_64-pc-windows-msvc', 'windows-aarch64': 'aarch64-pc-windows-msvc' }
const marker = '.substitute-native-build.json'

/** Never repurpose a prior target tree. Cargo profiles/features may coexist within a tree,
 * but its target, Rust pin, linker image and CRT linkage must retain the recorded identity.
 * The caller initializes its selected Visual Studio environment and supplies an explicit linker.
 */
export async function nativeConfiguration(directory, platform, workspace, environment) {
  if (!Object.hasOwn(targets, platform) || !path.isAbsolute(directory)) throw new Error('Explicit native build directory and platform required')
  const target = targets[platform]
  for (const name of ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_RUSTFLAGS', 'RUSTUP_TOOLCHAIN']) {
    if (environment[name]) throw new Error(`Native distribution builds use repository-controlled flags; unset ${name}`)
  }
  if (environment.CARGO_BUILD_TARGET && environment.CARGO_BUILD_TARGET !== target) throw new Error('Ambient Cargo target differs from the requested platform')
  const linkerVariable = `CARGO_TARGET_${target.replaceAll('-', '_').toUpperCase()}_LINKER`
  const linker = environment[linkerVariable]
  if (!linker || !path.isAbsolute(linker)) throw new Error(`Initialize the selected Visual Studio environment and set ${linkerVariable} to its absolute linker path`)
  await regularArtifact(linker)
  const digest = createHash('sha256')
  for await (const bytes of createReadStream(linker)) digest.update(bytes)
  const configuration = { schema_version: 1, target, rust_toolchain: await fs.readFile(path.join(workspace, 'rust-toolchain.toml'), 'utf8'), linker: { path: linker, sha256: digest.digest('hex') }, crt: 'static' }
  const record = path.join(directory, marker)
  try { await artifactDirectory(directory) } catch (cause) {
    if (cause.code !== 'ENOENT') throw cause
    await newArtifactDirectory(directory)
  }
  const entries = await fs.readdir(directory)
  if (entries.length && !entries.includes(marker)) throw new Error('Choose a fresh target directory; the existing build configuration is preserved')
  try { await fs.writeFile(record, JSON.stringify(configuration) + '\n', { flag: 'wx' }) } catch (cause) {
    if (cause.code !== 'EEXIST') throw cause
  }
  if ((await regularArtifact(record)).size > 8192) throw new Error('Native build configuration is oversized')
  const bytes = await fs.readFile(record, 'utf8')
  if (bytes.length > 8192 || JSON.stringify(JSON.parse(bytes)) !== JSON.stringify(configuration)) throw new Error('Native build configuration changed; choose a fresh target directory')
  return { target, environment: { ...environment, CARGO_ENCODED_RUSTFLAGS: '-C\x1ftarget-feature=+crt-static' }, configuration }
}
