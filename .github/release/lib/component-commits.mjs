// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * semantic-release plugin: analyse and describe only the commits that belong to one component.
 *
 * Options: `{ directory, standalone, analyzer, notes, publicRepositoryUrl }`. `directory` and
 * `standalone` are the component layout (see `component-scope.mjs`); `analyzer` and `notes` are
 * handed unchanged to `@semantic-release/commit-analyzer` and
 * `@semantic-release/release-notes-generator`. `publicRepositoryUrl`, when set, is the repository
 * the notes link to instead of the one semantic-release pushes to.
 */
import * as commitAnalyzer from '@semantic-release/commit-analyzer';
import * as notesGenerator from '@semantic-release/release-notes-generator';
import { memberCommits } from './component-scope.mjs';

const scopedByContext = new WeakMap();

/**
 * Decide the release type from the component's own commits.
 *
 * @param {Record<string, unknown>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 * @returns {Promise<string | null>} Release type, or null when no release is due.
 */
export async function analyzeCommits(options, context) {
  return commitAnalyzer.analyzeCommits(options.analyzer ?? {}, scoped(options, context));
}

/**
 * Write release notes from the component's own commits.
 *
 * @param {Record<string, unknown>} options Plugin options.
 * @param {Record<string, any>} context semantic-release context.
 * @returns {Promise<string>} Release notes.
 */
export async function generateNotes(options, context) {
  const commits = scoped(options, context);
  if (!options.publicRepositoryUrl) {
    return notesGenerator.generateNotes(options.notes ?? {}, commits);
  }
  return notesGenerator.generateNotes(options.notes ?? {}, {
    ...commits,
    options: { ...commits.options, repositoryUrl: options.publicRepositoryUrl },
  });
}

/** Copy the context with only member commits; the classification is shared by both steps. */
function scoped(options, context) {
  const layout = { directory: options.directory, standalone: options.standalone ?? [] };
  let commits = scopedByContext.get(context.commits);
  if (!commits) {
    const members = new Set(
      memberCommits(
        context.cwd,
        layout,
        context.commits.map((commit) => commit.hash),
      ),
    );
    commits = context.commits.filter((commit) => members.has(commit.hash));
    scopedByContext.set(context.commits, commits);
    context.logger.log(
      'Found %d of %d commits that belong to %s',
      commits.length,
      context.commits.length,
      layout.directory,
    );
  }
  return { ...context, commits };
}
