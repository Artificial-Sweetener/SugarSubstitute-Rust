// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Release one monorepo component, or plan which components have a release due.
 *
 *   node .github/release/release.mjs plan
 *   node .github/release/release.mjs release <sugarcubes|substitute> [--dry-run]
 *
 * `plan` needs no credentials: it reads tags and commits locally and runs the same commit
 * analysis semantic-release runs. `release` runs semantic-release from the component's folder.
 * Both write `key=value` lines to `$GITHUB_OUTPUT` when it is set, and print them otherwise.
 *
 * A dry run never touches the checkout or any remote: it runs the production configuration in a
 * disposable clone of HEAD that is its own and only remote, with no credentials in its
 * environment, so it cannot push, tag or publish anything, and it needs no permission to do so.
 *
 * Every component continues an existing version sequence: its newest release tag, or the
 * baseline it declares in `lib/components.mjs`. A component with neither stops with an
 * explanation instead of inventing a first version.
 */
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import * as commitAnalyzer from '@semantic-release/commit-analyzer';
import { ANALYZER, COMPONENTS, component, releaseOptions } from './lib/components.mjs';
import { memberCommits } from './lib/component-scope.mjs';

const VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const FULL_HASH = /^[0-9a-f]{40}([0-9a-f]{24})?$/;
/** The only branch semantic-release publishes from. */
const RELEASE_BRANCH = 'main';
/** Variables that would let a dry run detect CI, authenticate or push. */
const DRY_RUN_WITHHELD = ['CI', 'GITHUB_ACTIONS', 'GH_TOKEN', 'GITHUB_TOKEN', 'GIT_ASKPASS'];

/**
 * @typedef {object} Baseline
 * @property {string} tag Release tag of the baseline, existing or to be created.
 * @property {string} version Its version.
 * @property {string} commit Full hash of the released commit.
 * @property {boolean} declared True when no tag exists yet and the declared baseline is used.
 */

/**
 * Latest release of a component reachable from HEAD: its newest tag, else its declared baseline.
 *
 * @param {string} repository Repository root.
 * @param {import('./lib/components.mjs').Component} entry Component.
 * @returns {Baseline} Baseline release.
 */
export function baseline(repository, entry) {
  const versions = git(repository, ['tag', '--merged', 'HEAD', '--list', `${entry.tagPrefix}*`])
    .split('\n')
    .map((tag) => tag.trim())
    .filter((tag) => VERSION.test(tag.slice(entry.tagPrefix.length)))
    .map((tag) => ({ tag, version: tag.slice(entry.tagPrefix.length) }))
    .sort((left, right) => compare(right.version, left.version));
  const declared = entry.baseline;
  if (declared) {
    requireDeclaredBaseline(repository, entry);
    const tag = `${entry.tagPrefix}${declared.version}`;
    const tagged = git(repository, ['tag', '--list', tag]).trim();
    if (tagged !== '' && commitOf(repository, tag) !== declared.commit) {
      throw new Error(`${tag} does not tag the declared baseline commit ${declared.commit}.`);
    }
  }
  if (versions.length > 0) {
    const [newest] = versions;
    return { ...newest, commit: commitOf(repository, newest.tag), declared: false };
  }
  if (!declared) {
    throw new Error(
      `${entry.title} has no ${entry.tagPrefix}<version> tag reachable from HEAD and declares no ` +
        `baseline. Its first release continues an existing version: declare that release's ` +
        `version and commit as the component's baseline in .github/release/lib/components.mjs.`,
    );
  }
  return {
    tag: `${entry.tagPrefix}${declared.version}`,
    version: declared.version,
    commit: declared.commit,
    declared: true,
  };
}

/** A declared baseline must be a released commit in HEAD's history that declares its version. */
function requireDeclaredBaseline(repository, entry) {
  const { version, commit, versionFiles } = entry.baseline;
  if (!VERSION.test(version) || !FULL_HASH.test(commit) || versionFiles.length === 0) {
    throw new Error(`${entry.title} declares a malformed baseline.`);
  }
  const ancestor = spawnSync('git', ['merge-base', '--is-ancestor', commit, 'HEAD'], {
    cwd: repository,
    encoding: 'utf8',
    windowsHide: true,
  });
  if (ancestor.status !== 0) {
    throw new Error(
      `${entry.title}'s baseline commit ${commit} is not in HEAD's history` +
        (ancestor.status === 1 ? '.' : ` (${ancestor.stderr.trim()}); fetch the full history.`),
    );
  }
  const declaration = new RegExp(`^version = "${version.replaceAll('.', '\\.')}"$`, 'm');
  const present = versionFiles.filter((file) => exists(repository, `${commit}:${file}`));
  if (
    present.length === 0 ||
    present.some((file) => !declaration.test(git(repository, ['show', `${commit}:${file}`])))
  ) {
    throw new Error(
      `${entry.title}'s baseline commit ${commit} does not declare version ${version} in ` +
        `${versionFiles.join(' or ')}.`,
    );
  }
}

/**
 * Decide whether a component has a release due, using the analysis semantic-release uses.
 *
 * @param {string} repository Repository root.
 * @param {import('./lib/components.mjs').Component} entry Component.
 * @returns {Promise<{release: boolean, version: string, last: string, commits: number}>} Plan.
 */
export async function plan(repository, entry) {
  const last = baseline(repository, entry);
  const hashes = git(repository, ['rev-list', '--reverse', `${last.commit}..HEAD`])
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  const members = memberCommits(repository, entry, hashes);
  const commits = messages(repository, members);
  const type = await commitAnalyzer.analyzeCommits(ANALYZER, {
    cwd: join(repository, entry.directory),
    commits,
    logger: { log() {} },
  });
  return {
    release: type !== null,
    version: type === null ? '' : increment(last.version, type),
    last: last.version,
    commits: commits.length,
  };
}

/**
 * Run semantic-release for one component from its folder.
 *
 * A real run releases from the checkout and pushes to its remote; when the component's baseline
 * is still only declared, it first tags that baseline locally so semantic-release continues it
 * and pushes the tag with the release. A dry run uses a disposable clone instead (see above).
 *
 * @param {string} repository Repository root.
 * @param {import('./lib/components.mjs').Component} entry Component.
 * @param {{dryRun?: boolean, ci?: boolean, repositoryUrl?: string, publicRepositoryUrl?: string,
 *   env?: NodeJS.ProcessEnv, log?: NodeJS.WritableStream,
 *   plugins?: (plugins: unknown[]) => unknown[]}} [settings] Run settings; `log` receives
 *   semantic-release's output, and `plugins` lets a test replace publication steps.
 * @returns {Promise<{released: boolean, version: string, tag: string, notes: string}>} Outcome.
 */
export async function release(repository, entry, settings = {}) {
  const last = baseline(repository, entry);
  if (settings.dryRun) {
    return dryRun(repository, entry, last, settings);
  }
  if (last.declared) {
    git(repository, ['tag', last.tag, last.commit]);
  }
  return semanticRelease(repository, entry, settings);
}

/** Run the production configuration in a disposable clone that is its own only remote. */
async function dryRun(repository, entry, last, settings) {
  const folder = mkdtempSync(join(tmpdir(), 'release-dry-run-'));
  try {
    const clone = join(folder, 'repository');
    const head = git(repository, ['rev-parse', 'HEAD']).trim();
    git(folder, ['clone', '--quiet', '--no-checkout', '--shared', repository, clone]);
    git(clone, ['remote', 'remove', 'origin']);
    git(clone, ['checkout', '--quiet', '-B', RELEASE_BRANCH, head]);
    if (last.declared) {
      git(clone, ['tag', last.tag, last.commit]);
    }
    const env = { ...(settings.env ?? process.env) };
    for (const name of DRY_RUN_WITHHELD) {
      delete env[name];
    }
    return await semanticRelease(clone, entry, {
      ...settings,
      ci: false,
      env,
      repositoryUrl: pathToFileURL(clone).href,
      publicRepositoryUrl: settings.publicRepositoryUrl ?? publicRepositoryUrl(repository, env),
    });
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
}

async function semanticRelease(repository, entry, settings) {
  const { default: run } = await import('semantic-release');
  const options = releaseOptions(entry, {
    branch: RELEASE_BRANCH,
    repositoryUrl: settings.repositoryUrl,
    publicRepositoryUrl: settings.publicRepositoryUrl,
  });
  if (settings.plugins) {
    options.plugins = settings.plugins(options.plugins);
  }
  const result = await run(
    { ...options, dryRun: Boolean(settings.dryRun), ci: settings.ci ?? true },
    {
      cwd: join(repository, entry.directory),
      env: settings.env ?? process.env,
      stdout: settings.log ?? process.stdout,
      stderr: settings.log ?? process.stderr,
    },
  );
  if (!result || !result.nextRelease) {
    return { released: false, version: '', tag: '', notes: '' };
  }
  return {
    released: !settings.dryRun,
    version: result.nextRelease.version,
    tag: result.nextRelease.gitTag,
    notes: result.nextRelease.notes ?? '',
  };
}

/**
 * The public address release notes link to: the GitHub repository CI runs in, else the
 * checkout's `origin` without credentials, else none (the notes then link to the clone).
 */
function publicRepositoryUrl(repository, env) {
  if (env.GITHUB_SERVER_URL && env.GITHUB_REPOSITORY) {
    return `${env.GITHUB_SERVER_URL.replace(/\/$/, '')}/${env.GITHUB_REPOSITORY}`;
  }
  const remote = spawnSync('git', ['remote', 'get-url', 'origin'], {
    cwd: repository,
    encoding: 'utf8',
    windowsHide: true,
  });
  if (remote.status !== 0) {
    return undefined;
  }
  const address = remote.stdout.trim();
  const scp = /^[^@/]+@([^:/]+):(.+)$/.exec(address);
  if (scp) {
    return `https://${scp[1]}/${scp[2].replace(/\.git$/, '')}`;
  }
  try {
    const url = new URL(address);
    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      return undefined;
    }
    url.username = '';
    url.password = '';
    return url.href.replace(/\.git$/, '');
  } catch {
    return undefined;
  }
}

/** Commit messages for `hashes`, in order, from one git process. */
function messages(repository, hashes) {
  if (hashes.length === 0) {
    return [];
  }
  const result = spawnSync(
    'git',
    ['log', '--no-walk=unsorted', '--stdin', '--format=%H%x1f%B%x1e'],
    {
      cwd: repository,
      input: `${hashes.join('\n')}\n`,
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: 256 * 1024 * 1024,
    },
  );
  if (result.error || result.status !== 0) {
    throw result.error ?? new Error(`git log failed: ${result.stderr.trim()}`);
  }
  const commits = result.stdout
    .replace(/\r\n/g, '\n')
    .split('\u001e')
    .map((record) => record.replace(/^\n/, ''))
    .filter((record) => record.includes('\u001f'))
    .map((record) => {
      const [hash, message] = record.split('\u001f');
      return { hash, message: message.trim() };
    });
  if (
    commits.length !== hashes.length ||
    commits.some((commit, index) => commit.hash !== hashes[index])
  ) {
    throw new Error('git log did not return every requested commit message.');
  }
  return commits;
}

/** Next `major.minor.patch` for a release type. */
function increment(version, type) {
  const [major, minor, patch] = version.split('.').map(Number);
  switch (type) {
    case 'major':
      return `${major + 1}.0.0`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`Unexpected release type "${type}".`);
  }
}

/** The commit a ref names. */
function commitOf(repository, ref) {
  return git(repository, ['rev-parse', `${ref}^{commit}`]).trim();
}

/** Whether `object` (such as `<commit>:<path>`) exists. */
function exists(repository, object) {
  return (
    spawnSync('git', ['cat-file', '-e', object], { cwd: repository, windowsHide: true }).status ===
    0
  );
}

function compare(left, right) {
  const a = left.split('.').map(Number);
  const b = right.split('.').map(Number);
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

function git(repository, args) {
  const result = spawnSync('git', args, {
    cwd: repository,
    encoding: 'utf8',
    windowsHide: true,
    maxBuffer: 256 * 1024 * 1024,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`git ${args[0]} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.replace(/\r\n/g, '\n');
}

/** Write outputs for the workflow, or print them for a person. */
function emit(values) {
  const lines = Object.entries(values).map(([key, value]) => `${key}=${value}`);
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `${lines.join('\n')}\n`, 'utf8');
  }
  process.stdout.write(`${lines.join('\n')}\n`);
}

/**
 * Refuse a real release unless the release workflow runs it with publication switched on.
 * The workflow checks the same switch; this keeps a local or mistaken run from publishing.
 *
 * @param {NodeJS.ProcessEnv} env Environment of this process.
 */
export function requirePublicationEnabled(env) {
  if (env.GITHUB_ACTIONS !== 'true') {
    throw new Error('Releases publish only from the release workflow; use --dry-run locally.');
  }
  if (env.RELEASES_ENABLED !== 'true') {
    throw new Error(
      'Publication is switched off: the repository variable RELEASES_ENABLED is not "true". ' +
        'Use --dry-run to preview a release.',
    );
  }
}

async function main(argv) {
  const repository = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
  const [command, name, ...flags] = argv;
  if (command === 'plan' && name === undefined) {
    const outputs = {};
    for (const entry of Object.values(COMPONENTS)) {
      let due;
      try {
        due = await plan(repository, entry);
      } catch (cause) {
        // One component without a baseline must not hold back the other's release; the
        // workflow reports that component as blocked with this reason instead.
        outputs[`${entry.name}_release`] = 'false';
        outputs[`${entry.name}_version`] = '';
        outputs[`${entry.name}_blocked`] = String(cause.message ?? cause).replace(/\r?\n/g, ' ');
        process.stderr.write(`${entry.title}: ${outputs[`${entry.name}_blocked`]}\n`);
        continue;
      }
      outputs[`${entry.name}_release`] = String(due.release);
      outputs[`${entry.name}_version`] = due.version;
      outputs[`${entry.name}_blocked`] = '';
      process.stderr.write(
        `${entry.title}: ${due.commits} commits since ${due.last}; ` +
          `${due.release ? `next release ${due.version}` : 'no release due'}\n`,
      );
    }
    emit(outputs);
    return;
  }
  if (command === 'release' && name !== undefined && flags.every((flag) => flag === '--dry-run')) {
    const entry = component(name);
    const dryRun = flags.includes('--dry-run');
    if (!dryRun) {
      requirePublicationEnabled(process.env);
    }
    const outcome = await release(repository, entry, { dryRun });
    if (outcome.version) {
      const folder = join(repository, 'dist', entry.name);
      mkdirSync(folder, { recursive: true });
      writeFileSync(join(folder, 'notes.md'), outcome.notes, 'utf8');
    }
    emit({ released: String(outcome.released), version: outcome.version, tag: outcome.tag });
    return;
  }
  throw new Error('Usage: release.mjs plan | release.mjs release <component> [--dry-run]');
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((cause) => {
    process.stderr.write(`${cause.stack ?? cause}\n`);
    process.exitCode = 1;
  });
}
