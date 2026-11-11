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
/** Retired region input cleanup owns only its original draft projection, independently of newer host intent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { regionDraftDecorationSets } from '../transaction/region-draft-observation.ts';
import { undoDepth, redoDepth } from '@codemirror/commands';
import {
  regionDraft,
  setRegionDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/region-draft.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

/** Export only the actual region owner for controlled release callbacks; no production statements are replaced. */
const hook = registerHooks({
  load(url, context, next) {
    const result = next(url, context);
    return url ===
      new URL('../../../../../../frontend/sugarprompt/editor/src/region-name.ts', import.meta.url)
        .href
      ? {
          ...result,
          source: readFileSync(new URL(url), 'utf8') + '\nexport { RegionNames };\n',
        }
      : result;
  },
});
/** Test observation contains no editable region state beyond the owner's own visible input boundary. */
interface RegionOwner {
  target: object | null;
  input: { element: { value: string }; destroy(): void } | null;
  finish(cancel: boolean, outside: boolean): void;
  destroy(): void;
}
const { RegionNames } = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/region-name.ts'
)) as unknown as {
  RegionNames: new (view: EditorView) => RegionOwner;
};
const { hostFixture } = await import('../transaction/host-document-fixture.ts');

for (const cancel of [false, true])
  test(`${cancel ? 'cancelled' : 'accepted'} region release removes only the retired draft while newer selection/draft/disposal wins`, () => {
    const previousWindow = globalThis.window;
    globalThis.window = { removeEventListener() {} } as unknown as Window & typeof globalThis;
    try {
      for (const later of ['selection', 'draft', 'destroy'] as const) {
        const host = hostFixture('[SEP|Old]\nbody'),
          view = host.transport as unknown as EditorView,
          owner = new RegionNames(view);
        const freshness = host.state().field(editorFreshness),
          target = host.state().field(semanticStructures).native!.regions[0]!;
        const draft = {
            freshness,
            scopeId: target.scopeId,
            ordinal: 0,
            width: 42,
          },
          newer = { freshness, scopeId: target.scopeId, ordinal: 0, width: 77 };
        host.transport.dispatch({ effects: setRegionDraft.of(draft) });
        owner.target = { target, freshness };
        let released = false,
          cleanup = 0;
        const dispatch = host.transport.dispatch.bind(host.transport);
        host.transport.dispatch = (value) => {
          if (released) {
            cleanup++;
            const transaction = value instanceof Transaction ? value : host.state().update(value);
            assert.equal(transaction.docChanged, false);
            assert.equal(transaction.selection, undefined);
            assert.equal(transaction.annotation(Transaction.addToHistory), false);
          }
          for (const transaction of Array.isArray(value) ? value : [value])
            dispatch(transaction as Transaction | TransactionSpec);
        };
        owner.input = {
          element: { value: 'New' },
          destroy() {
            dispatch({ selection: { anchor: 5, head: 1 } });
            if (later === 'draft') {
              dispatch({ effects: setRegionDraft.of(newer) });
              owner.target = { target, freshness };
            }
            if (later === 'destroy') owner.destroy();
            released = true;
          },
        };
        owner.finish(cancel, false);
        assert.equal(cleanup, later === 'selection' ? 1 : 0);
        assert.equal(host.source() === '[SEP|Old]\nbody', true);
        assert.deepEqual(
          [host.state().selection.main.anchor, host.state().selection.main.head],
          [5, 1],
        );
        assert.deepEqual([undoDepth(host.state()), redoDepth(host.state())], [0, 0]);
        assert.equal(
          host.state().field(regionDraft),
          later === 'selection' ? null : later === 'draft' ? newer : draft,
        );
        let editing = false;
        for (const decorations of regionDraftDecorationSets(host.state(), view)) {
          decorations.between(0, host.state().doc.length, (_from, _to, decoration) => {
            if (decoration.spec.attributes?.class === 'sp-region-editing') editing = true;
          });
        }
        assert.equal(editing, later !== 'selection');
        owner.destroy();
      }
    } finally {
      globalThis.window = previousWindow;
    }
  });

test('regional cleanup publication preserves a callback-selected reverse range, replacement draft and disposal', () => {
  const previousWindow = globalThis.window;
  globalThis.window = { removeEventListener() {} } as unknown as Window & typeof globalThis;
  try {
    for (const later of ['selection', 'draft', 'destroy'] as const) {
      const host = hostFixture('[SEP|Old]\nbody'),
        view = host.transport as unknown as EditorView,
        owner = new RegionNames(view);
      const freshness = host.state().field(editorFreshness),
        target = host.state().field(semanticStructures).native!.regions[0]!;
      const original = {
          freshness,
          scopeId: target.scopeId,
          ordinal: 0,
          width: 42,
        },
        newer = { freshness, scopeId: target.scopeId, ordinal: 0, width: 88 };
      host.transport.dispatch({ effects: setRegionDraft.of(original) });
      owner.target = { target, freshness };
      owner.input = { element: { value: 'New' }, destroy() {} };
      let publications = 0;
      const dispatch = host.transport.dispatch.bind(host.transport);
      host.transport.dispatch = (value) => {
        publications++;
        dispatch(value);
        dispatch({ selection: { anchor: 5, head: 1 } });
        if (later === 'draft') {
          dispatch({ effects: setRegionDraft.of(newer) });
          owner.target = { target, freshness };
        }
        if (later === 'destroy') owner.destroy();
      };
      owner.finish(true, false);
      assert.equal(publications, 1);
      assert.equal(host.source() === '[SEP|Old]\nbody', true);
      assert.deepEqual(
        [host.state().selection.main.anchor, host.state().selection.main.head],
        [5, 1],
      );
      assert.deepEqual([undoDepth(host.state()), redoDepth(host.state())], [0, 0]);
      assert.equal(host.state().field(regionDraft), later === 'draft' ? newer : null);
      owner.destroy();
    }
  } finally {
    globalThis.window = previousWindow;
  }
});

test.after(() => hook.deregister());
