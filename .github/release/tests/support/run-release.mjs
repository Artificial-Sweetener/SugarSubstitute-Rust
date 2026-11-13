// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Run one component's real semantic-release in its own process for a test.
 *
 * semantic-release hooks process.stdout while it runs; inside the test runner's process that
 * would rewrite the runner's own reports. Usage: node run-release.mjs <request.json>, where the
 * request holds `{ root, component, baseline, dryRun, record, env, publicRepositoryUrl }`.
 * `baseline`, when present, replaces the component's declared baseline (null for none) so it can
 * name a synthetic commit; `env` is added to the environment. With `record`, the GitHub publication step is replaced by one that
 * writes the release it was given to that file; everything else is the production
 * configuration. The outcome is printed as JSON on the last line.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { COMPONENTS } from '../../lib/components.mjs';
import { release } from '../../release.mjs';
import { SYNTHETIC_REPOSITORY } from './repository.mjs';

/**
 * The test's own environment without the CI service it may run in: semantic-release would
 * otherwise take the branch and credentials of the CI run, not of the synthetic repository.
 * A test that means to run as CI passes those variables in `request.env`.
 */
function outsideCi(env) {
  return Object.fromEntries(
    Object.entries(env).filter(
      ([name]) => name !== 'CI' && name !== 'GH_TOKEN' && !name.startsWith('GITHUB_'),
    ),
  );
}

const GITHUB_RELEASE = fileURLToPath(new URL('../../lib/github-release.mjs', import.meta.url));
const request = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const recordPublication = {
  publish: async (_options, context) => {
    writeFileSync(request.record, JSON.stringify(context.nextRelease), 'utf8');
    return { name: 'recorded', url: 'about:blank' };
  },
};
const production = COMPONENTS[request.component];
const entry = Object.hasOwn(request, 'baseline')
  ? { ...production, baseline: request.baseline }
  : production;
const outcome = await release(request.root, entry, {
  ci: false,
  dryRun: request.dryRun,
  // A file URL, not a bare path: the notes generator parses the repository URL, and only a
  // Windows drive path happens to parse as one.
  repositoryUrl: pathToFileURL(request.root).href,
  env: { ...outsideCi(process.env), GITHUB_REPOSITORY: SYNTHETIC_REPOSITORY, ...request.env },
  publicRepositoryUrl: request.publicRepositoryUrl,
  log: process.stderr,
  plugins: request.record
    ? (plugins) =>
        plugins.map((plugin) =>
          Array.isArray(plugin) && plugin[0] === GITHUB_RELEASE ? [recordPublication, {}] : plugin,
        )
    : undefined,
});
process.stdout.write(`${JSON.stringify(outcome)}\n`);
