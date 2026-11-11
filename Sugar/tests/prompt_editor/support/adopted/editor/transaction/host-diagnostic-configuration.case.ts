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
/** Real native host adoption preserves source/history and retires replaced external diagnostic capabilities. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, undoDepth, redoDepth } from '@codemirror/commands';
import { hostFixture } from './host-document-fixture.ts';
import type { HostUpdate } from '../../../../../../frontend/sugarprompt/editor/src/host-update.ts';
import { booruPostLookup } from '../../../../../../frontend/sugarprompt/editor/src/booru-paste-host.ts';
import { spellingSession } from '../../../../../../frontend/sugarprompt/editor/src/spelling-diagnostics.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  SpellcheckSession,
  type BooruPostLookup,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

/** A genuine empty dictionary adapter validates session construction without duplicating service findings. */
function dictionary(): SpellcheckSession {
  return new SpellcheckSession({
    identity: Symbol(),
    languageTag: 'en',
    availability: () => ({
      status: 'available',
      dictionaryAdd: false,
      sessionIgnore: false,
    }),
    checkWords: async (words) => ({
      status: 'ready',
      value: words.map(() => true),
    }),
    suggest: async () => ({ status: 'ready', value: [] }),
  });
}

test('lookup and shared dictionary adoption preserve the entire native edit and redo branches', () => {
  const owner = hostFixture('a\r\nb'),
    session = dictionary();
  const lookup: BooruPostLookup = async () => ({ status: 'unavailable' });
  try {
    owner.updates.apply({ mode: 'exact-edit', source: 'first' });
    owner.updates.apply({ mode: 'exact-edit', source: 'second' });
    undo(owner.transport);
    owner.updates.apply({
      mode: 'preserve',
      selection: { anchor: 5, head: 1 },
    });
    const prior = owner.state(),
      freshness = prior.field(editorFreshness);
    assert.equal(
      owner.updates.apply({
        mode: 'preserve',
        configuration: { booruPostLookup: lookup, spellcheckSession: session },
      }).status,
      'applied',
    );
    const installed = owner.state();
    assert.equal(owner.source(), 'first');
    assert.deepEqual([installed.selection.main.anchor, installed.selection.main.head], [5, 1]);
    assert.deepEqual([undoDepth(installed), redoDepth(installed)], [1, 1]);
    assert.strictEqual(installed.facet(booruPostLookup), lookup);
    assert.strictEqual(installed.facet(spellingSession), session);
    assert.notStrictEqual(installed.field(editorFreshness), freshness);
    const counts = owner.counts();
    assert.equal(
      owner.updates.apply({
        mode: 'preserve',
        configuration: { booruPostLookup: lookup, spellcheckSession: session },
      }).status,
      'unchanged',
    );
    assert.strictEqual(owner.state(), installed);
    assert.deepEqual(owner.counts(), counts);
    assert.equal(
      owner.updates.apply({
        mode: 'preserve',
        configuration: { booruPostLookup: null, spellcheckSession: null },
      }).status,
      'applied',
    );
    const removed = owner.state();
    assert.equal(removed.facet(booruPostLookup), null);
    assert.equal(removed.facet(spellingSession), null);
    assert.notStrictEqual(removed.field(editorFreshness), installed.field(editorFreshness));
    assert.equal(owner.source(), 'first');
    assert.deepEqual([removed.selection.main.anchor, removed.selection.main.head], [5, 1]);
    assert.deepEqual([undoDepth(removed), redoDepth(removed)], [1, 1]);
  } finally {
    session.dispose();
    owner.destroy();
  }
});

test('malformed or prototype-only capabilities reject the complete bundle before native authority changes', () => {
  for (const configuration of [
    { booruPostLookup: undefined },
    { booruPostLookup: false },
    { booruPostLookup: {} },
    { spellcheckSession: undefined },
    { spellcheckSession: false },
    { spellcheckSession: {} },
    {
      spellcheckSession: Object.create(SpellcheckSession.prototype) as unknown,
    },
  ]) {
    const owner = hostFixture();
    try {
      owner.updates.apply({ mode: 'exact-edit', source: 'first' });
      owner.updates.apply({ mode: 'exact-edit', source: 'second' });
      undo(owner.transport);
      const prior = owner.state(),
        values = owner.configuration.values,
        counts = owner.counts();
      assert.throws(
        () =>
          owner.updates.apply({
            mode: 'baseline',
            source: 'replacement',
            configuration: {
              ...configuration,
              placeholder: 'new',
              readOnly: true,
            },
          } as unknown as HostUpdate),
        TypeError,
      );
      assert.strictEqual(owner.state(), prior);
      assert.strictEqual(owner.configuration.values, values);
      assert.deepEqual(owner.counts(), counts);
      assert.equal(owner.permission.matches(false), true);
      assert.deepEqual([undoDepth(prior), redoDepth(prior)], [1, 1]);
    } finally {
      owner.destroy();
    }
  }
});
