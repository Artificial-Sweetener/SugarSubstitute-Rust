// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
/**
 * Plan and dry-run against this repository's own history, with the production configuration.
 *
 * Needs the complete history (`fetch-depth: 0` in CI): the SugarCubes baseline is a commit of its
 * imported standalone history. The assertions are invariants that hold for any HEAD.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { COMPONENTS } from '../lib/components.mjs';
import { memberCommits } from '../lib/component-scope.mjs';
import { baseline, plan } from '../release.mjs';
import { git } from './support/repository.mjs';

const MONOREPO = fileURLToPath(new URL('../../../', import.meta.url));
const RUNNER = fileURLToPath(new URL('./support/run-release.mjs', import.meta.url));
const PUBLIC = 'https://github.com/Artificial-Sweetener/SugarSubstitute-Rust';

describe('this repository', () => {
  let folder;
  before(() => {
    folder = mkdtempSync(join(tmpdir(), 'release-monorepo-'));
  });
  after(() => rmSync(folder, { recursive: true, force: true }));

  it('continues SugarCubes from its 0.15.3 release commit', () => {
    const last = baseline(MONOREPO, COMPONENTS.sugarcubes);
    assert.equal(last.version >= '0.15.3', true);
    if (last.declared) {
      assert.deepEqual(last, {
        tag: 'sugarcubes-v0.15.3',
        version: '0.15.3',
        commit: COMPONENTS.sugarcubes.baseline.commit,
        declared: true,
      });
      assert.equal(
        git(MONOREPO, ['log', '-1', '--format=%s', last.commit]),
        'chore(release): 0.15.3 [skip ci]',
      );
    }
  });

  it('dry-runs the SugarCubes release the plan predicts, from SugarCubes commits only', async () => {
    const entry = COMPONENTS.sugarcubes;
    const due = await plan(MONOREPO, entry);
    const request = join(folder, 'request.json');
    writeFileSync(
      request,
      JSON.stringify({
        root: MONOREPO,
        component: 'sugarcubes',
        dryRun: true,
        env: { GITHUB_REPOSITORY: 'Artificial-Sweetener/SugarSubstitute-Rust' },
        publicRepositoryUrl: PUBLIC,
      }),
      'utf8',
    );
    const before = git(MONOREPO, ['for-each-ref', '--format=%(refname) %(objectname)']);
    const result = spawnSync(process.execPath, [RUNNER, request], {
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: 256 * 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr.slice(-4000));
    const outcome = JSON.parse(result.stdout.trim().split('\n').at(-1));
    assert.equal(outcome.version, due.version);
    assert.equal(outcome.released, false);
    assert.equal(git(MONOREPO, ['for-each-ref', '--format=%(refname) %(objectname)']), before);
    if (!due.release) {
      return;
    }
    assert.equal(outcome.tag, `sugarcubes-v${due.version}`);
    const last = baseline(MONOREPO, entry);
    const range = git(MONOREPO, ['rev-list', `${last.commit}..HEAD`]).split('\n');
    const members = new Set(memberCommits(MONOREPO, entry, range));
    const cited = [...outcome.notes.matchAll(/\/commit\/([0-9a-f]{40})\)/g)].map(
      ([, hash]) => hash,
    );
    assert.ok(cited.length > 0);
    for (const hash of cited) {
      assert.ok(members.has(hash), `the notes cite ${hash}, which is not a SugarCubes commit`);
    }
  });
});
