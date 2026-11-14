// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/** Produce a release build's update distribution once the generated changelog describes it. */
import fs from 'node:fs/promises'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { fileURLToPath, pathToFileURL } from 'node:url'

/**
 * The distribution every release build compiles in: repository, feed, policy, notes link and the
 * repository's trusted root.
 * Its repository identity is the SHA-256 of
 * `sugarsubstitute-update-repository:github.com/Artificial-Sweetener/SugarSubstitute-Rust/substitute-updates`
 * and never changes once a release ships it, because installations bind their trust to it.
 */
export const releaseDistribution = fileURLToPath(new URL('./update-distribution.json', import.meta.url))
/** The root metadata the update repository was created with; installed trust can restart from it. */
export const repositoryRoot = fileURLToPath(new URL('../../../.github/update-repository/root.json', import.meta.url))
const workspaceRoot = fileURLToPath(new URL('../../', import.meta.url))
const distributionLimit = 16 * 1024
const changelogLimit = 16 * 1024 ** 2
const placeholder = '{version}'
const plainVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/

/** The `[workspace.package]` version: the exact version the workspace builds. */
export async function workspaceVersion(workspace) {
  const manifest = await fs.readFile(path.join(workspace, 'Cargo.toml'), 'utf8')
  const section = manifest.match(/^\[workspace\.package\]\r?\n([\s\S]*?)(?=^\[|(?![\s\S]))/m)
  const version = section?.[1].match(/^version = "([^"]+)"\r?$/m)?.[1]
  if (!version || !plainVersion.test(version)) throw new Error('Cargo.toml [workspace.package] declares no release version')
  return version
}

/** The changelog section semantic-release wrote for exactly `version`, heading included. */
export function changelogSection(changelog, version) {
  const escaped = version.replaceAll('.', '\\.')
  const heading = new RegExp(`^(#{1,3}) (?:\\[${escaped}\\]\\(|${escaped} )`, 'm')
  const match = heading.exec(changelog)
  if (!match) throw new Error(`CHANGELOG.md has no section for ${version}; only a released version is built with updates`)
  const rest = changelog.slice(match.index + match[0].length)
  const next = new RegExp(`^#{1,${match[1].length}} `, 'm').exec(rest)
  const section = changelog.slice(match.index, match.index + match[0].length + (next ? next.index : rest.length)).trimEnd()
  if (!section.includes('\n')) throw new Error(`CHANGELOG.md lists ${version} without any notes`)
  return section
}

/**
 * Validate the committed distribution as far as release tooling can: one https notes template and
 * the repository's trusted root, exactly as the update repository commits it.
 */
function requireReleaseNotes(distribution, version, trustedRoot) {
  const template = distribution?.release_notes
  if (distribution?.schema_version !== 4 || typeof template !== 'string' || template.split(placeholder).length !== 2) {
    throw new Error('The release distribution needs schema 4 and one release notes template with a version placeholder')
  }
  if (distribution.trusted_root !== trustedRoot) {
    throw new Error('The release distribution must carry the update repository trusted root exactly')
  }
  const link = new URL(template.replace(placeholder, version))
  if (link.protocol !== 'https:' || link.username || link.password || link.search || !link.hostname) {
    throw new Error('Release notes must link to a plain https address')
  }
  return link.href
}

/**
 * Write the update distribution for the version `workspace` builds, after its changelog section
 * is confirmed. The bytes are the committed release distribution, unchanged; its notes link opens
 * the published release whose body is that section. `output` must not exist.
 */
export async function writeUpdateDistribution({ workspace = workspaceRoot, source = releaseDistribution, root = repositoryRoot, output }) {
  if (!path.isAbsolute(output ?? '')) throw new Error('The update distribution needs a new absolute output path')
  const version = await workspaceVersion(workspace)
  const changelogFile = path.join(workspace, 'CHANGELOG.md')
  if ((await fs.stat(changelogFile)).size > changelogLimit) throw new Error('CHANGELOG.md is larger than release tooling reads')
  const section = changelogSection(await fs.readFile(changelogFile, 'utf8'), version)
  const bytes = await fs.readFile(source)
  if (bytes.length === 0 || bytes.length > distributionLimit) throw new Error('The release distribution is empty or exceeds its compiled bound')
  const trustedRoot = new TextDecoder('utf-8', { fatal: true }).decode(await fs.readFile(root))
  const releaseNotes = requireReleaseNotes(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)), version, trustedRoot)
  await fs.writeFile(output, bytes, { flag: 'wx' })
  const sha256 = data => createHash('sha256').update(data).digest('hex')
  return { distribution: output, version, releaseNotes, sha256: sha256(bytes), changelogSha256: sha256(section) }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [output, ...extra] = process.argv.slice(2)
  if (!output || extra.length) throw new Error('Usage: node installer/release/update-distribution.mjs <new-absolute-output>; then build with SUGARSUBSTITUTE_UPDATE_DISTRIBUTION set to it')
  console.log(JSON.stringify(await writeUpdateDistribution({ output })))
}
