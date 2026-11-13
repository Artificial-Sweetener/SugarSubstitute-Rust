// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Build throwaway git repositories with the monorepo's three historical layouts.
 *
 * Repositories live in a fresh folder under the system temporary directory and never have a
 * remote: semantic-release is pointed at the repository's own path, so its fetch and push are
 * no-ops against the same refs.
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

let clock = 1_700_000_000;

/** Identity and fixed dates so repositories are reproducible and need no global git config. */
export function gitEnvironment() {
  clock += 60;
  const date = `${clock} +0000`;
  return {
    ...process.env,
    GIT_AUTHOR_NAME: 'Release Test',
    GIT_AUTHOR_EMAIL: 'release-test@example.invalid',
    GIT_COMMITTER_NAME: 'Release Test',
    GIT_COMMITTER_EMAIL: 'release-test@example.invalid',
    GIT_AUTHOR_DATE: date,
    GIT_COMMITTER_DATE: date,
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: process.platform === 'win32' ? 'NUL' : '/dev/null',
  };
}

/**
 * Run git in `cwd` and return its trimmed output.
 *
 * @param {string} cwd Working directory.
 * @param {string[]} args Arguments.
 * @returns {string} Standard output.
 */
export function git(cwd, args) {
  const result = spawnSync(
    'git',
    ['-c', 'init.defaultBranch=main', '-c', 'core.autocrlf=false', ...args],
    {
      cwd,
      env: gitEnvironment(),
      encoding: 'utf8',
      windowsHide: true,
    },
  );
  if (result.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout.trim();
}

/** A disposable repository; call `dispose()` when the test ends. */
export class Repository {
  constructor() {
    this.root = mkdtempSync(join(tmpdir(), 'release-repo-'));
    git(this.root, ['init', '--quiet']);
  }

  /** Write files (`{ path: content }`) and commit them with `message`; returns the commit hash. */
  commit(message, files, remove = []) {
    for (const [path, content] of Object.entries(files)) {
      const target = join(this.root, path);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, content, 'utf8');
    }
    for (const path of remove) {
      git(this.root, ['rm', '-r', '--quiet', path]);
    }
    git(this.root, ['add', '--all']);
    git(this.root, ['commit', '--quiet', '--allow-empty', '-m', message]);
    return git(this.root, ['rev-parse', 'HEAD']);
  }

  /** Merge `branch` into the current branch with the given strategy options. */
  merge(branch, message, options = []) {
    git(this.root, [
      'merge',
      '--quiet',
      '--no-ff',
      '--allow-unrelated-histories',
      ...options,
      '-m',
      message,
      branch,
    ]);
    return git(this.root, ['rev-parse', 'HEAD']);
  }

  dispose() {
    rmSync(this.root, { recursive: true, force: true });
  }
}

/** Repository the synthetic releases are tagged in; the synthetic release source names it. */
export const SYNTHETIC_REPOSITORY = 'example/monorepo';

/**
 * A component as configured for production, with its declared baseline moved onto the
 * synthetic history: SugarCubes declares its synthetic 0.15.3 release, SugarSubstitute none.
 *
 * @param {import('../../lib/components.mjs').Component} entry Production component.
 * @param {Record<string, string>} commits Named commits of `monorepoHistory()`.
 * @returns {import('../../lib/components.mjs').Component} Component for the synthetic history.
 */
export function synthetic(entry, commits) {
  if (!entry.baseline) {
    return entry;
  }
  return { ...entry, baseline: { ...entry.baseline, commit: commits.sugarRelease } };
}

/**
 * The monorepo history in miniature: a standalone Rust app on `main`, a standalone SugarCubes on
 * `sugarcubes`, the layout move, and a SugarCubes subtree merge with commits that keep their
 * root-layout trees.
 *
 * @returns {{repository: Repository, commits: Record<string, string>}} Repository and named commits.
 */
export function monorepoHistory() {
  const repository = new Repository();
  const commits = {};
  commits.rustStart = repository.commit('build(workspace): establish workspace', {
    'Cargo.toml': '[workspace]\nmembers = ["crates/app"]\n',
    'rust-toolchain.toml': '[toolchain]\nchannel = "1.93.1"\n',
    'crates/app/src/main.rs': 'fn main() {}\n',
  });
  git(repository.root, ['checkout', '--quiet', '--orphan', 'sugarcubes']);
  git(repository.root, ['rm', '-r', '--quiet', '--cached', '.']);
  git(repository.root, ['clean', '-fdq']);
  commits.sugarRelease = repository.commit('chore(release): 0.15.3 [skip ci]', {
    'pyproject.toml': 'version = "0.15.3"\n',
    '__init__.py': '__version__ = "0.15.3"\n',
    'sugarcubes/nodes.py': 'NODES = {}\n',
    'sugarcubes/backend/self_update/release_source.json': JSON.stringify({
      schema: 1,
      packId: 'SugarCubes',
      githubRelease: {
        repository: 'example/monorepo',
        tagPrefix: 'sugarcubes-v',
        assetName: 'SugarCubes-{version}.zip',
      },
    }),
  });
  git(repository.root, ['tag', 'sugarcubes-v0.15.3']);
  commits.sugarFeature = repository.commit('feat(nodes): add a cube', {
    'sugarcubes/nodes.py': 'NODES = {"cube": 1}\n',
  });
  git(repository.root, ['checkout', '--quiet', 'main']);
  commits.rustFix = repository.commit('fix(desktop): keep the window', {
    'crates/app/src/main.rs': 'fn main() { }\n',
  });
  commits.layout = repository.commit(
    'refactor(repository): move the Rust workspace into Substitute',
    {
      'Substitute/Cargo.toml': '[workspace]\nmembers = ["crates/app"]\n',
      'Substitute/rust-toolchain.toml': '[toolchain]\nchannel = "1.93.1"\n',
      'Substitute/crates/app/src/main.rs': 'fn main() { }\n',
      'AGENTS.md': 'Monorepo\n',
    },
    ['Cargo.toml', 'rust-toolchain.toml', 'crates'],
  );
  git(repository.root, [
    'merge',
    '--quiet',
    '-s',
    'ours',
    '--no-commit',
    '--allow-unrelated-histories',
    'sugarcubes',
  ]);
  git(repository.root, ['read-tree', '--prefix=Sugar/', '-u', 'sugarcubes']);
  git(repository.root, [
    'commit',
    '--quiet',
    '-m',
    'build(repository): import SugarCubes history into Sugar',
  ]);
  commits.import = git(repository.root, ['rev-parse', 'HEAD']);
  git(repository.root, ['checkout', '--quiet', 'sugarcubes']);
  commits.sugarFix = repository.commit('fix(nodes): keep cube names', {
    'sugarcubes/nodes.py': 'NODES = {"cube": 2}\n',
  });
  git(repository.root, ['checkout', '--quiet', 'main']);
  commits.subtreeMerge = repository.merge(
    'sugarcubes',
    'merge: bring SugarCubes main into the monorepo layout',
    ['-X', 'subtree=Sugar'],
  );
  return { repository, commits };
}
