// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Decide which commits belong to one component of the monorepo.
 *
 * The repository carries three layouts in its history: monorepo commits with `Sugar/` and
 * `Substitute/` side by side, and the imported standalone histories of SugarCubes and the Rust
 * app, whose files sit at the tree root. Subtree merges keep the imported commits' original
 * trees, so a plain path filter on `Sugar/` would miss every SugarCubes commit made before the
 * move. Each commit is therefore judged by its own tree:
 *
 * - a tree that contains the component directory is a monorepo commit, and belongs when its
 *   change against the first parent touches that directory;
 * - a tree with every standalone marker at its root is the component's standalone history, and
 *   belongs when it changes anything;
 * - any other tree belongs to another component.
 */
import { spawnSync } from 'node:child_process';

const RECORD = '\u001e';
const MAX_OUTPUT_BYTES = 512 * 1024 * 1024;

/**
 * @typedef {object} ComponentLayout
 * @property {string} directory Component folder relative to the repository root, such as `Sugar`.
 * @property {string[]} standalone Root entries that identify the component's standalone history.
 */

/**
 * Return the subset of `hashes` that belong to the component, in their original order.
 *
 * @param {string} repository Absolute path inside the git work tree.
 * @param {ComponentLayout} layout Component layout.
 * @param {string[]} hashes Full commit hashes to classify.
 * @returns {string[]} Hashes of member commits.
 */
export function memberCommits(repository, layout, hashes) {
  validateLayout(layout);
  if (hashes.length === 0) {
    return [];
  }
  for (const hash of hashes) {
    if (!/^[0-9a-f]{40}([0-9a-f]{24})?$/.test(hash)) {
      throw new Error(`Expected a full commit hash, got "${hash}".`);
    }
  }
  const present = probeEntries(repository, hashes, [layout.directory, ...layout.standalone]);
  const changed = changedPaths(repository, hashes);
  const prefix = `${layout.directory}/`;
  return hashes.filter((hash) => {
    const entries = present.get(hash);
    const paths = changed.get(hash) ?? [];
    if (entries.has(layout.directory)) {
      return paths.some((path) => path.startsWith(prefix));
    }
    if (layout.standalone.every((marker) => entries.has(marker))) {
      return paths.length > 0;
    }
    return false;
  });
}

/** Reject layouts that could match the whole repository or escape it. */
function validateLayout(layout) {
  const names = [layout.directory, ...layout.standalone];
  if (layout.standalone.length === 0) {
    throw new Error('A component layout needs at least one standalone marker.');
  }
  for (const name of names) {
    if (typeof name !== 'string' || !/^[A-Za-z0-9_][A-Za-z0-9_.-]*$/.test(name)) {
      throw new Error(`Layout entries must be single root names, got "${name}".`);
    }
  }
}

/** Map each commit to the subset of `names` present at its tree root, in one git process. */
function probeEntries(repository, hashes, names) {
  const queries = hashes.flatMap((hash) => names.map((name) => `${hash}:${name}`));
  const output = git(
    repository,
    ['cat-file', '--batch-check=%(objecttype)'],
    `${queries.join('\n')}\n`,
  );
  const lines = output.split('\n').slice(0, queries.length);
  if (lines.length !== queries.length) {
    throw new Error('git cat-file returned an incomplete answer.');
  }
  const present = new Map(hashes.map((hash) => [hash, new Set()]));
  lines.forEach((line, index) => {
    if (line.endsWith(' missing')) {
      return;
    }
    if (line !== 'tree' && line !== 'blob') {
      throw new Error(`Unexpected git cat-file answer "${line}".`);
    }
    const hash = hashes[Math.floor(index / names.length)];
    present.get(hash).add(names[index % names.length]);
  });
  return present;
}

/** Map each commit to the paths it changes against its first parent (all paths for a root). */
function changedPaths(repository, hashes) {
  const output = git(
    repository,
    [
      'log',
      '--no-walk=unsorted',
      '--stdin',
      '--diff-merges=first-parent',
      '--no-renames',
      '--name-only',
      `--format=${RECORD}%H`,
    ],
    `${hashes.join('\n')}\n`,
  );
  const changed = new Map();
  for (const record of output.split(RECORD).slice(1)) {
    const [hash, ...paths] = record.split('\n');
    changed.set(hash.trim(), paths.map((path) => path.trim()).filter(Boolean));
  }
  for (const hash of hashes) {
    if (!changed.has(hash)) {
      throw new Error(`git log did not describe commit ${hash}.`);
    }
  }
  return changed;
}

/** Run git with an argument list; never through a shell. */
function git(repository, args, input) {
  const result = spawnSync('git', ['-c', 'core.quotePath=false', ...args], {
    cwd: repository,
    input,
    encoding: 'utf8',
    maxBuffer: MAX_OUTPUT_BYTES,
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`git ${args[0]} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.replace(/\r\n/g, '\n');
}
