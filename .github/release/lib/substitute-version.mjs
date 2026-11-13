// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Write a SugarSubstitute release version into the Rust workspace.
 *
 * The workspace version in `Cargo.toml` is the single declared version; every member inherits it
 * through `version.workspace = true`. `Cargo.lock` records it once per workspace member, so each
 * of those entries moves with it. Path dependencies outside the workspace keep their own versions.
 * Run with the component folder (`Substitute/`) as the working directory.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

/**
 * Update the workspace manifest and lockfile in `workspace` to `version`.
 *
 * @param {string} workspace Rust workspace folder.
 * @param {string} version Release version, `major.minor.patch`.
 */
export function writeSubstituteVersion(workspace, version) {
  if (!VERSION.test(version)) {
    throw new Error(`Expected a release version like 1.2.3, got "${version}".`);
  }
  const manifestPath = join(workspace, 'Cargo.toml');
  const manifest = readFileSync(manifestPath, 'utf8');
  const section = manifest.match(/^\[workspace\.package\]\n([\s\S]*?)(?=^\[|(?![\s\S]))/m);
  if (!section) {
    throw new Error('Cargo.toml has no [workspace.package] section.');
  }
  const declared = section[1].match(/^version = "([^"]+)"$/m);
  if (!declared) {
    throw new Error('Cargo.toml [workspace.package] declares no version.');
  }
  const members = workspaceMembers(workspace, manifest);
  const updatedSection = section[1].replace(/^version = "[^"]+"$/m, `version = "${version}"`);

  const lockPath = join(workspace, 'Cargo.lock');
  const lock = readFileSync(lockPath, 'utf8');
  const blocks = lock.split(/(?=^\[\[package\]\]$)/m);
  let moved = 0;
  const updated = blocks.map((block) => {
    const name = block.match(/^name = "([^"]+)"$/m)?.[1];
    if (!name || !members.has(name) || /^source = /m.test(block)) {
      return block;
    }
    const current = block.match(/^version = "([^"]+)"$/m)?.[1];
    if (current !== declared[1]) {
      throw new Error(`Cargo.lock records ${name} at ${current}, not the declared ${declared[1]}.`);
    }
    moved += 1;
    return block.replace(/^version = "[^"]+"$/m, `version = "${version}"`);
  });
  if (moved !== members.size) {
    throw new Error(`Cargo.lock lists ${moved} of ${members.size} workspace members.`);
  }
  // Both files are checked before either is written, so a refused update changes nothing.
  writeFileSync(manifestPath, manifest.replace(section[1], updatedSection), 'utf8');
  writeFileSync(lockPath, updated.join(''), 'utf8');
}

/** Names of workspace members that inherit the workspace version. */
function workspaceMembers(workspace, manifest) {
  const list = manifest.match(/^members = \[([^\]]*)\]/m);
  if (!list) {
    throw new Error('Cargo.toml lists no workspace members.');
  }
  const names = new Set();
  for (const [, member] of list[1].matchAll(/"([^"]+)"/g)) {
    const memberManifest = readFileSync(join(workspace, member, 'Cargo.toml'), 'utf8');
    const name = memberManifest.match(/^name = "([^"]+)"$/m)?.[1];
    if (!name) {
      throw new Error(`${member}/Cargo.toml has no package name.`);
    }
    if (!/^version\.workspace = true$/m.test(memberManifest)) {
      throw new Error(`${member}/Cargo.toml must inherit the workspace version.`);
    }
    names.add(name);
  }
  return names;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  writeSubstituteVersion(process.cwd(), process.argv[2] ?? '');
}
