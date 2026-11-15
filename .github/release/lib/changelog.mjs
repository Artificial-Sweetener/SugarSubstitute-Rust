// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * semantic-release plugin: write a version's changelog section once.
 *
 * A release can stop after its commit reached the branch and before its tag or publication,
 * for instance when the remote refuses a tag. The next run plans the same version again. Its
 * section is already committed, so this plugin leaves the changelog alone; the version files
 * are already written too, so nothing is committed, and the run tags and publishes the existing
 * release commit. Every other run is `@semantic-release/changelog`, unchanged.
 *
 * Options: `changelogFile`, as for `@semantic-release/changelog`.
 */
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import changelog from '@semantic-release/changelog';

/**
 * Whether a changelog already has the section semantic-release writes for a version.
 *
 * @param {string} text Changelog text.
 * @param {string} version Release version, `major.minor.patch`.
 * @returns {boolean} True when a heading of that version is present.
 */
export function hasSection(text, version) {
  const escaped = version.replaceAll('.', '\\.');
  return new RegExp(`^#{1,3} (?:\\[${escaped}\\]\\(|${escaped} )`, 'm').test(text);
}

/**
 * Check the configuration as `@semantic-release/changelog` does.
 *
 * @param {Record<string, any>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 */
export async function verifyConditions(options, context) {
  await changelog.verifyConditions(options, context);
}

/**
 * Write the next release's section unless the changelog already has it.
 *
 * @param {Record<string, any>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 */
export async function prepare(options, context) {
  const file = resolve(context.cwd, options.changelogFile);
  const { version } = context.nextRelease;
  if (existsSync(file) && hasSection(readFileSync(file, 'utf8'), version)) {
    context.logger.log(
      '%s already describes %s: completing the release that wrote it.',
      options.changelogFile,
      version,
    );
    return;
  }
  await changelog.prepare(options, context);
}
