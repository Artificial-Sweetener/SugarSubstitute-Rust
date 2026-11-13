// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';
import { COMPONENTS } from '../lib/components.mjs';
import { memberCommits } from '../lib/component-scope.mjs';
import { baseline, plan } from '../release.mjs';
import { git, monorepoHistory, synthetic } from './support/repository.mjs';

describe('component commit scope', () => {
  let history;
  let all;
  before(() => {
    history = monorepoHistory();
    history.monoSugar = history.repository.commit('feat(panel): add a panel', {
      'Sugar/frontend/panel.ts': 'x\n',
    });
    history.monoSubstitute = history.repository.commit('fix(update): retry', {
      'Substitute/crates/app/src/main.rs': 'fn main() { retry() }\n',
    });
    history.monoBoth = history.repository.commit('feat(contracts): add a capability', {
      'Sugar/integrations/contracts/canvas.json': '{}\n',
      'Substitute/crates/app/src/contract.rs': '\n',
    });
    history.rootOnly = history.repository.commit('ci: add a workflow', {
      '.github/workflows/ci.yml': 'on: push\n',
    });
    all = git(history.repository.root, ['rev-list', '--reverse', 'HEAD']).split('\n');
  });
  after(() => history.repository.dispose());

  it('keeps standalone SugarCubes commits and Sugar/ changes for SugarCubes', () => {
    const { commits } = history;
    const members = memberCommits(history.repository.root, COMPONENTS.sugarcubes, all);
    assert.deepEqual(
      new Set(members),
      new Set([
        commits.sugarRelease,
        commits.sugarFeature,
        commits.import,
        commits.sugarFix,
        commits.subtreeMerge,
        history.monoSugar,
        history.monoBoth,
      ]),
    );
  });

  it('keeps standalone Rust commits and Substitute/ changes for SugarSubstitute', () => {
    const { commits } = history;
    const members = memberCommits(history.repository.root, COMPONENTS.substitute, all);
    assert.deepEqual(
      new Set(members),
      new Set([
        commits.rustStart,
        commits.rustFix,
        commits.layout,
        history.monoSubstitute,
        history.monoBoth,
      ]),
    );
  });

  it('gives a change outside both components to neither', () => {
    for (const entry of Object.values(COMPONENTS)) {
      assert.equal(memberCommits(history.repository.root, entry, [history.rootOnly]).length, 0);
    }
  });

  it('plans a SugarCubes minor release from its own feature only', async () => {
    const entry = synthetic(COMPONENTS.sugarcubes, history.commits);
    const due = await plan(history.repository.root, entry);
    assert.deepEqual(
      { release: due.release, version: due.version, last: due.last },
      { release: true, version: '0.16.0', last: '0.15.3' },
    );
  });

  it('refuses to plan a component with no baseline tag', async () => {
    await assert.rejects(
      plan(history.repository.root, COMPONENTS.substitute),
      /no substitute-v<version> tag reachable from HEAD and declares no baseline/,
    );
  });

  it('uses the newest tag once one exists, and checks a declared baseline against it', () => {
    const root = history.repository.root;
    const entry = synthetic(COMPONENTS.sugarcubes, history.commits);
    assert.deepEqual(baseline(root, entry), {
      tag: 'sugarcubes-v0.15.3',
      version: '0.15.3',
      commit: history.commits.sugarRelease,
      declared: false,
    });
    const moved = { ...entry, baseline: { ...entry.baseline, commit: history.commits.sugarFix } };
    assert.throws(() => baseline(root, moved), /does not tag the declared baseline commit/);
  });

  it('refuses a declared baseline outside HEAD or that does not declare its version', () => {
    const root = history.repository.root;
    const entry = synthetic(COMPONENTS.sugarcubes, history.commits);
    const declare = (change) => ({ ...entry, baseline: { ...entry.baseline, ...change } });
    git(root, ['checkout', '--quiet', '-b', 'elsewhere', history.commits.rustFix]);
    const outside = history.repository.commit('fix(nodes): elsewhere', {
      'pyproject.toml': 'version = "0.15.3"\n',
    });
    git(root, ['checkout', '--quiet', 'main']);
    assert.throws(() => baseline(root, declare({ commit: outside })), /not in HEAD's history/);
    assert.throws(
      () => baseline(root, declare({ version: '0.15.4', commit: history.commits.sugarRelease })),
      /does not declare version 0\.15\.4/,
    );
    assert.throws(
      () => baseline(root, declare({ versionFiles: ['missing.toml'] })),
      /does not declare version 0\.15\.3 in missing\.toml/,
    );
    assert.throws(() => baseline(root, declare({ commit: 'abc123' })), /malformed baseline/);
    assert.throws(() => baseline(root, declare({ version: 'v0.15.3' })), /malformed baseline/);
  });

  it('rejects abbreviated hashes and layouts that could match the whole tree', () => {
    assert.throws(
      () => memberCommits(history.repository.root, COMPONENTS.sugarcubes, ['abc123']),
      /full commit hash/,
    );
    assert.throws(
      () =>
        memberCommits(history.repository.root, { directory: '..', standalone: ['x'] }, [
          history.rootOnly,
        ]),
      /single root names/,
    );
    assert.throws(
      () =>
        memberCommits(history.repository.root, { directory: 'Sugar', standalone: [] }, [
          history.rootOnly,
        ]),
      /standalone marker/,
    );
  });
});
