//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Independent literals qualify host bundles, final offsets, native history and rejected-state identity. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth, redoDepth } from '@codemirror/commands';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { hostFixture } from './host-document-fixture.ts';
import type { HostUpdate } from '../../../../../../frontend/sugarprompt/editor/src/host-update.ts';

test('whole-source exact editing defaults to final exact end, one isolated native undo event, equal-source no-op', () => {
  const owner = hostFixture();
  assert.equal(
    owner.updates.apply({ mode: 'exact-edit', source: '🙂\r\nx\ry\n\ud800' }).status,
    'applied',
  );
  assert.equal(owner.source(), '🙂\r\nx\ry\n\ud800');
  assert.equal(owner.state().selection.main.head, 8);
  assert.equal(undoDepth(owner.state()), 1);
  const same = owner.state();
  assert.equal(
    owner.updates.apply({ mode: 'exact-edit', source: '🙂\r\nx\ry\n\ud800' }).status,
    'unchanged',
  );
  assert.strictEqual(owner.state(), same);
  assert.equal(undo(owner.transport), true);
  assert.equal(owner.source(), 'old');
  assert.equal(redo(owner.transport), true);
  assert.equal(owner.source(), '🙂\r\nx\ry\n\ud800');
});

test('privileged equal-byte baseline clears both history branches and refreshes native source identity at complete end', () => {
  const owner = hostFixture('a\r\n🙂', true);
  const previous = owner.state().field(editorFreshness).source;
  assert.equal(
    owner.updates.apply({
      mode: 'baseline',
      source: 'a\r\n🙂',
      configuration: {
        documentKind: 'wildcard-txt',
        readOnly: true,
        richRendering: false,
      },
    }).status,
    'applied',
  );
  assert.equal(owner.state().selection.main.head, 4);
  assert.equal(owner.state().readOnly, true);
  assert.equal(owner.state().facet(documentKind), 'wildcard-txt');
  assert.notStrictEqual(owner.state().field(editorFreshness).source, previous);
  assert.equal(owner.revision(), 1);
  assert.equal(undoDepth(owner.state()), 0);
  assert.equal(redoDepth(owner.state()), 0);
  assert.equal(
    owner.updates.apply({
      mode: 'baseline',
      source: 'a\r\n🙂',
      selection: { anchor: 5, head: 1 },
    }).status,
    'applied',
  );
  assert.equal(owner.state().selection.main.anchor, 4);
  assert.equal(owner.state().selection.main.head, 1);
});

test('baseline discards redo and undo independently after prior exact replacement', () => {
  const owner = hostFixture();
  owner.updates.apply({ mode: 'exact-edit', source: 'first' });
  owner.updates.apply({ mode: 'exact-edit', source: 'second' });
  assert.equal(undo(owner.transport), true);
  assert.equal(undoDepth(owner.state()), 1);
  assert.equal(redoDepth(owner.state()), 1);
  owner.updates.apply({ mode: 'baseline', source: 'first' });
  assert.equal(undo(owner.transport), false);
  assert.equal(redo(owner.transport), false);
});

test('invalid final selection/configuration leaves every authority and pending intent untouched', () => {
  const owner = hostFixture();
  const state = owner.state(),
    config = owner.configuration.values,
    counts = owner.counts();
  for (const update of [
    {
      mode: 'baseline',
      source: 'a\r\nb',
      selection: { anchor: 2, head: 0 },
      configuration: { documentKind: 'wildcard-txt', readOnly: true },
    },
    { mode: 'baseline', source: 'valid', configuration: { placeholder: 7 } },
    { mode: 'preserve', configuration: { readOnly: null } },
    { mode: 'preserve', configuration: null },
    { mode: 'baseline', source: 'valid', selection: null },
    { mode: 'preserve', selection: null },
    { mode: 'baseline', source: 'a', selection: { anchor: 2, head: 0 } },
  ]) {
    assert.throws(() => owner.updates.apply(update as unknown as HostUpdate));
    assert.strictEqual(owner.state(), state);
    assert.strictEqual(owner.configuration.values, config);
    assert.equal(owner.revision(), 0);
    assert.deepEqual(owner.counts(), counts);
  }
});

test('source-neutral host selection and permission inherit input restoration; source and explicit focus replace it', () => {
  const owner = hostFixture('old', true);
  owner.updates.apply({ mode: 'preserve', configuration: { readOnly: false } });
  owner.updates.apply({ mode: 'preserve', selection: { anchor: 3, head: 1 } });
  owner.updates.apply({ mode: 'exact-edit', source: 'next' });
  owner.updates.apply({ mode: 'baseline', source: 'next' });
  owner.updates.apply({ mode: 'preserve', focus: 'editor' });
  assert.deepEqual(owner.admissions(), [
    'preserve-input',
    'preserve-input',
    'replace-input',
    'replace-input',
    'replace-input',
  ]);
});

test('source-bearing permission revocation rejects native startup; no retained source queue', () => {
  const owner = hostFixture();
  owner.transport.compositionStarted = true;
  const before = owner.state();
  for (const mode of ['baseline', 'exact-edit'] as const)
    assert.deepEqual(
      owner.updates.apply({
        mode,
        source: 'next',
        configuration: { readOnly: true },
      }),
      { status: 'rejected', reason: 'composing' },
    );
  assert.strictEqual(owner.state(), before);
  assert.equal(owner.revision(), 0);
  owner.transport.compositionStarted = false;
  assert.equal(owner.source(), 'old');
});

test('placeholder is literal configuration and reuses actual prepared semantic/profile objects without history/source changes', () => {
  const owner = hostFixture('plain');
  const profile = resolveFeatureProfile(defaultPreferences, {
    mode: 'library',
  });
  owner.updates.apply({
    mode: 'preserve',
    configuration: { featureProfile: profile },
  });
  const native = owner.state().field(semanticStructures).native;
  owner.updates.apply({ mode: 'preserve', selection: { anchor: 4, head: 1 } });
  const before = owner.state();
  owner.updates.apply({
    mode: 'preserve',
    configuration: { placeholder: '<IMG SRC=X>' },
  });
  assert.strictEqual(owner.state().field(semanticStructures).native, native);
  assert.strictEqual(owner.state().facet(featureProfile), profile);
  assert.equal(owner.source(), 'plain');
  assert.equal(owner.state().selection.main.anchor, 4);
  assert.equal(owner.state().selection.main.head, 1);
  assert.equal(undoDepth(owner.state()), undoDepth(before));
  assert.equal(owner.revision(), 0);
  owner.updates.apply({ mode: 'preserve', configuration: { placeholder: '' } });
  assert.strictEqual(owner.state().field(semanticStructures).native, native);
});

test('synchronous later host baseline or destruction wins before older requested focus', () => {
  for (const destroy of [false, true]) {
    const owner = hostFixture();
    let once = true;
    owner.callback(() => {
      if (!once) return;
      once = false;
      if (destroy) owner.destroy();
      else
        owner.updates.apply({
          mode: 'baseline',
          source: 'later',
          selection: { anchor: 4, head: 1 },
        });
    });
    owner.updates.apply({ mode: 'baseline', source: 'early', focus: 'editor' });
    assert.equal(owner.counts().focuses, 0);
    if (!destroy) {
      assert.equal(owner.source(), 'later');
      assert.equal(owner.state().selection.main.anchor, 4);
      assert.equal(owner.state().selection.main.head, 1);
    }
  }
});

test('equal-byte baseline rejects prior prepared source handles and catalog replacement retires context', async () => {
  const { PreparedSourceEdits } = await import(
    '../../../../../../frontend/sugarprompt/editor/src/prepared-edits.ts'
  );
  const { prepareSceneTitleCatalog } = await import(
    '../../../../../../frontend/sugarprompt/services/src/index.ts'
  );
  for (const baseline of [true, false]) {
    const native = hostFixture('old'),
      owner = new PreparedSourceEdits();
    const prepared = owner.prepare(native.state(), false, 0, 1, 'x');
    assert.equal(prepared.status, 'ready');
    if (prepared.status !== 'ready') throw new Error('Preparation unavailable');
    if (baseline) native.updates.apply({ mode: 'baseline', source: 'old' });
    else
      native.updates.apply({
        mode: 'preserve',
        configuration: {
          sceneTitleCatalog: prepareSceneTitleCatalog({
            status: 'ready',
            revision: 'next',
            titles: ['Scene'],
          }),
        },
      });
    assert.equal(owner.take(native.state(), false, prepared.command).status, 'rejected');
    owner.dispose();
  }
});
