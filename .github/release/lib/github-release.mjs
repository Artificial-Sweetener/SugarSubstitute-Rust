// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * semantic-release plugin: publish a component's GitHub release with the GitHub CLI.
 *
 * Options:
 * - `title`: release title prefix, such as `SugarCubes`.
 * - `assets`: asset paths relative to the component, with `{version}` for the release version.
 * - `releaseSource`: optional path to a SugarCubes `release_source.json`. When present, the
 *   repository it names must be the one the release is tagged in (`GITHUB_REPOSITORY`), and its
 *   tag prefix and asset name must match this release, so the updater finds exactly what CI
 *   uploads.
 *
 * The release is created with `--verify-tag`: GitHub refuses it unless the tag semantic-release
 * pushed is already in the target repository, so a release can never mint a tag on other code.
 * After upload, every asset's GitHub-recorded sha256 must equal the local file's digest.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';

const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;

/**
 * Check the configuration in every run, and publishing credentials in a real run.
 *
 * @param {Record<string, any>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 */
export async function verifyConditions(options, context) {
  target(options, context);
  if (context.options.dryRun) {
    return;
  }
  if (!context.env.GH_TOKEN && !context.env.GITHUB_TOKEN) {
    throw new Error('Publishing a GitHub release needs GH_TOKEN or GITHUB_TOKEN.');
  }
  run(['--version'], context);
}

/**
 * Create the release, upload its assets and confirm GitHub's recorded digests.
 *
 * @param {Record<string, any>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 * @returns {Promise<{name: string, url: string}>} Published release.
 */
export async function publish(options, context) {
  const { repository } = target(options, context);
  const { version, gitTag, notes } = context.nextRelease;
  const assets = assetPaths(options, context.cwd, version);
  for (const asset of assets) {
    if (!statSync(asset).isFile()) {
      throw new Error(`Release asset ${asset} is not a file.`);
    }
  }
  const title = `${options.title} ${version}`;
  const folder = mkdtempSync(join(tmpdir(), 'release-notes-'));
  try {
    const notesFile = join(folder, 'notes.md');
    writeFileSync(notesFile, notes ?? '', 'utf8');
    run(
      [
        'release',
        'create',
        gitTag,
        ...assets,
        '--repo',
        repository,
        '--title',
        title,
        '--notes-file',
        notesFile,
        '--verify-tag',
      ],
      context,
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
  const release = JSON.parse(
    run(['api', `repos/${repository}/releases/tags/${encodeURIComponent(gitTag)}`], context),
  );
  requireRecordedDigests(release, assets);
  context.logger.log('Published %s to %s', title, release.html_url);
  return { name: title, url: release.html_url };
}

/**
 * Require that GitHub recorded each uploaded asset with the local file's sha256.
 *
 * @param {{assets?: Array<{name: string, digest?: string | null}>}} release GitHub release JSON.
 * @param {string[]} assets Local asset paths.
 */
export function requireRecordedDigests(release, assets) {
  for (const asset of assets) {
    const name = basename(asset);
    const recorded = (release.assets ?? []).find((entry) => entry.name === name);
    const expected = `sha256:${createHash('sha256').update(readFileSync(asset)).digest('hex')}`;
    if (!recorded) {
      throw new Error(`GitHub does not list the uploaded asset ${name}.`);
    }
    if (recorded.digest !== expected) {
      throw new Error(
        `GitHub recorded ${recorded.digest ?? 'no digest'} for ${name}; the uploaded file is ${expected}.`,
      );
    }
  }
}

/**
 * Resolve the publication repository and check a release source against this release.
 *
 * @param {Record<string, any>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 * @returns {{repository: string}} Publication target.
 */
export function target(options, context) {
  if (typeof options.title !== 'string' || options.title.trim() === '') {
    throw new Error('The GitHub release publisher needs a title.');
  }
  if (!Array.isArray(options.assets)) {
    throw new Error('The GitHub release publisher needs an assets list.');
  }
  if (!options.releaseSource) {
    const repository = context.env.GITHUB_REPOSITORY;
    if (!context.options.dryRun && !validRepository(repository)) {
      throw new Error('GITHUB_REPOSITORY must name the repository that receives the release.');
    }
    return { repository };
  }
  const source = JSON.parse(readFileSync(resolve(context.cwd, options.releaseSource), 'utf8'));
  const github = source.githubRelease ?? {};
  if (!validRepository(github.repository)) {
    throw new Error(`${options.releaseSource} does not name a GitHub repository.`);
  }
  // semantic-release pushes the tag to the repository the workflow runs in, and the release is
  // created with --verify-tag, so the updater's repository must be that same repository.
  const running = context.env.GITHUB_REPOSITORY;
  if (!context.options.dryRun && !validRepository(running)) {
    throw new Error('GITHUB_REPOSITORY must name the repository that receives the release.');
  }
  if (running !== undefined && running.toLowerCase() !== github.repository.toLowerCase()) {
    throw new Error(
      `${options.releaseSource} sends the updater to ${github.repository}, but this release is ` +
        `tagged in ${running}.`,
    );
  }
  if (`${github.tagPrefix}\${version}` !== context.options.tagFormat) {
    throw new Error(
      `${options.releaseSource} expects tags "${github.tagPrefix}<version>", but releases are tagged "${context.options.tagFormat}".`,
    );
  }
  const names = options.assets.map((asset) => basename(asset));
  if (!names.includes(github.assetName)) {
    throw new Error(
      `${options.releaseSource} expects the asset "${github.assetName}", which this release does not upload.`,
    );
  }
  return { repository: github.repository };
}

/** Expand `{version}` in each configured asset path, relative to the component. */
function assetPaths(options, cwd, version) {
  return options.assets.map((asset) => resolve(cwd, asset.replaceAll('{version}', version)));
}

function validRepository(value) {
  return typeof value === 'string' && /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value);
}

/** Run the GitHub CLI with an argument list and return its standard output. */
function run(args, context) {
  const result = spawnSync('gh', args, {
    cwd: context.cwd,
    env: context.env,
    encoding: 'utf8',
    maxBuffer: MAX_OUTPUT_BYTES,
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`gh ${args[0]} ${args[1] ?? ''} failed: ${result.stderr.trim()}`);
  }
  return result.stdout;
}
