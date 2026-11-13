// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * The two independently released components and their semantic-release configuration.
 *
 * Both follow the same rules as the earlier Python app and SugarCubes: features and breaking
 * changes bump the minor version, fixes and performance changes bump the patch version. Each
 * component has its own tag namespace, changelog and version files, and analyses only its own
 * commits, so a change to one component never releases the other. Release commits carry no
 * notes in their body: the notes are in the changelog and the GitHub release, and a first
 * release after hundreds of commits would exceed the command-line limit git is invoked with.
 */
import { fileURLToPath } from 'node:url';

const COMPONENT_COMMITS = fileURLToPath(new URL('./component-commits.mjs', import.meta.url));
const GITHUB_RELEASE = fileURLToPath(new URL('./github-release.mjs', import.meta.url));
const SUBSTITUTE_VERSION = fileURLToPath(new URL('./substitute-version.mjs', import.meta.url));
const REGISTRY_ARCHIVE = fileURLToPath(new URL('../sugarcubes_registry.py', import.meta.url));

/** Release rules shared by every component; unmatched commit types fall back to no release. */
export const RELEASE_RULES = Object.freeze([
  { breaking: true, release: 'minor' },
  { type: 'feat', release: 'minor' },
  { type: 'fix', release: 'patch' },
  { type: 'perf', release: 'patch' },
]);

/** Commit-analyzer options shared by release planning and semantic-release. */
export const ANALYZER = { preset: 'conventionalcommits', releaseRules: RELEASE_RULES };
// Notes use the same preset as the analysis, so `type!:` breaking changes are listed as such.
// The preset stays on major 9: major 10 needs a newer changelog writer than the generator ships.
const NOTES = { preset: 'conventionalcommits' };

/**
 * @typedef {object} Component
 * @property {string} name Component identifier used on the command line.
 * @property {string} title Product name shown in release titles.
 * @property {string} directory Component folder at the repository root.
 * @property {string[]} standalone Root entries that identify its imported standalone history.
 * @property {string} tagPrefix Tag namespace; tags are `<prefix><version>`.
 * @property {{version: string, commit: string, versionFiles: string[]} | null} baseline The last
 *   release published before this repository tagged it, or null when the component has none.
 *   `versionFiles` are the files, at the commit's tree root, that must declare `version`.
 * @property {() => unknown[]} plugins semantic-release plugins after commit analysis and notes.
 */

/** @type {Readonly<Record<string, Component>>} */
export const COMPONENTS = Object.freeze({
  sugarcubes: {
    name: 'sugarcubes',
    title: 'SugarCubes',
    directory: 'Sugar',
    standalone: ['pyproject.toml', '__init__.py', 'sugarcubes'],
    tagPrefix: 'sugarcubes-v',
    // SugarCubes 0.15.3 was released from its standalone repository, whose tags were not
    // imported. Its release commit is part of this history, so the first release continues it.
    baseline: {
      version: '0.15.3',
      commit: 'a839f5935fd96ca3bbd707ef817943d1fa2a4e6a',
      versionFiles: ['pyproject.toml', 'Sugar/pyproject.toml'],
    },
    plugins: () => [
      ['@semantic-release/changelog', { changelogFile: 'CHANGELOG.md' }],
      [
        '@semantic-release/exec',
        {
          prepareCmd:
            'npx tsx scripts/update-release-versions.ts ${nextRelease.version}' +
            ` && python ${JSON.stringify(REGISTRY_ARCHIVE)} pack` +
            ' --output ../dist/sugarcubes/SugarCubes-${nextRelease.version}.zip',
        },
      ],
      [
        '@semantic-release/git',
        {
          assets: [
            'package.json',
            'package-lock.json',
            'pyproject.toml',
            '__init__.py',
            'CHANGELOG.md',
          ],
          message: 'chore(release): sugarcubes ${nextRelease.version} [skip ci]',
        },
      ],
      [
        GITHUB_RELEASE,
        {
          title: 'SugarCubes',
          releaseSource: 'sugarcubes/backend/self_update/release_source.json',
          assets: ['../dist/sugarcubes/SugarCubes-{version}.zip'],
        },
      ],
    ],
  },
  substitute: {
    name: 'substitute',
    title: 'SugarSubstitute',
    directory: 'Substitute',
    standalone: ['Cargo.toml', 'crates', 'rust-toolchain.toml'],
    tagPrefix: 'substitute-v',
    // SugarSubstitute has never been released; its first version is the maintainer's decision.
    baseline: null,
    plugins: () => [
      ['@semantic-release/changelog', { changelogFile: 'CHANGELOG.md' }],
      [
        '@semantic-release/exec',
        { prepareCmd: `node ${JSON.stringify(SUBSTITUTE_VERSION)} \${nextRelease.version}` },
      ],
      [
        '@semantic-release/git',
        {
          assets: ['CHANGELOG.md', 'Cargo.toml', 'Cargo.lock'],
          message: 'chore(release): substitute ${nextRelease.version} [skip ci]',
        },
      ],
      [GITHUB_RELEASE, { title: 'SugarSubstitute', assets: [] }],
    ],
  },
});

/**
 * Look up a component by name.
 *
 * @param {string} name Component identifier.
 * @returns {Component} The component.
 */
export function component(name) {
  const found = Object.hasOwn(COMPONENTS, name) ? COMPONENTS[name] : undefined;
  if (!found) {
    throw new Error(
      `Unknown component "${name}"; expected one of ${Object.keys(COMPONENTS).join(', ')}.`,
    );
  }
  return found;
}

/**
 * Complete semantic-release options for one component.
 *
 * @param {Component} entry Component.
 * @param {{repositoryUrl?: string, branch?: string, publicRepositoryUrl?: string}} [overrides]
 *   Repository and branch; `publicRepositoryUrl` is the address release notes link to when the
 *   release runs against another location, such as a dry run's disposable clone.
 * @returns {Record<string, unknown>} semantic-release options.
 */
export function releaseOptions(entry, overrides = {}) {
  return {
    branches: [overrides.branch ?? 'main'],
    ...(overrides.repositoryUrl ? { repositoryUrl: overrides.repositoryUrl } : {}),
    tagFormat: `${entry.tagPrefix}\${version}`,
    plugins: [
      [
        COMPONENT_COMMITS,
        {
          directory: entry.directory,
          standalone: entry.standalone,
          analyzer: ANALYZER,
          notes: NOTES,
          ...(overrides.publicRepositoryUrl
            ? { publicRepositoryUrl: overrides.publicRepositoryUrl }
            : {}),
        },
      ],
      ...entry.plugins(),
    ],
  };
}
