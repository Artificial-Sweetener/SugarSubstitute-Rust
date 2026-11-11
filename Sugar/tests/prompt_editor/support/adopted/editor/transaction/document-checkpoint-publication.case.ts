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
/** Actual derived publication preserves document capture while native authority changes reject it. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { isolateHistory } from '@codemirror/commands';
import { EditorSelection, Transaction } from '@codemirror/state';
import { checkpointFixture } from './document-checkpoint-fixture.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { nativeDocumentSeed } from '../../../../../../frontend/sugarprompt/editor/src/document-state.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { installAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { preparedAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';

/** Publish the real canonical semantic and projection effects, never a stand-in state identity. */
function publish(f: ReturnType<typeof checkpointFixture>): void {
  const before = f.state(),
    seed = nativeDocumentSeed(before);
  const semantic = prepareNativeStructure(before);
  f.transport.dispatch({
    effects: [
      installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
      installProjection.of(prepareProjection(before, semantic)),
    ],
    annotations: Transaction.addToHistory.of(false),
  });
  const after = nativeDocumentSeed(f.state());
  assert.notEqual(f.state(), before);
  for (const key of ['doc', 'selection', 'endings', 'history', 'authorship'] as const)
    assert.equal(after[key], seed[key]);
  assert.equal(f.state().field(editorFreshness), before.field(editorFreshness));
}

for (const phase of ['before-read', 'between-read-write', 'dispatch', 'cleanup'] as const) {
  test(`capture admits canonical derived publication at ${phase}`, async () => {
    const f = checkpointFixture('a\r\nb');
    try {
      if (phase === 'cleanup')
        Object.assign(f.transport, {
          plugin: () => ({
            hasCompositionInput: () => false,
            reserveCommand: () => ({ finish() {}, cancel: () => publish(f) }),
          }),
        });
      const pending = f.updates.captureDocument();
      if (phase === 'before-read') publish(f);
      if (phase === 'between-read-write') {
        const measure = f.measurements.shift()!;
        const ready = measure.read(f.transport as unknown as Parameters<typeof measure.read>[0]);
        publish(f);
        measure.write?.(ready, f.transport as unknown as Parameters<typeof measure.read>[0]);
      } else {
        if (phase === 'dispatch') {
          const dispatch = f.transport.dispatch;
          let once = true;
          Object.assign(f.transport, {
            dispatch: (...args: Parameters<typeof dispatch>) => {
              dispatch(...args);
              if (once) {
                once = false;
                publish(f);
              }
            },
          });
        }
        f.flush();
      }
      const outcome = await pending;
      assert.equal(outcome.status, 'captured');
      if (outcome.status === 'captured') {
        assert.equal(outcome.checkpoint.source === 'a\r\nb', true);
        outcome.checkpoint.release();
      }
    } finally {
      f.dispose();
    }
  });
}

for (const transition of [
  'source',
  'selection',
  'endings',
  'history',
  'authorship',
  'context',
] as const) {
  for (const phase of ['before-read', 'between-read-write', 'cleanup'] as const) {
    test(`capture rejects real ${transition} authority change at ${phase}`, async () => {
      const f = checkpointFixture('a\r\nb');
      /** Change a genuine captured field or current command context through supported transactions. */
      const change = () => {
        if (transition === 'history') {
          const before = nativeDocumentSeed(f.state());
          f.transport.dispatch({
            annotations: [isolateHistory.of('before'), Transaction.addToHistory.of(false)],
          });
          const after = nativeDocumentSeed(f.state());
          assert.notEqual(after.history, before.history);
          assert.equal(after.doc, before.doc);
          assert.equal(after.selection, before.selection);
          assert.equal(after.endings, before.endings);
          assert.equal(after.authorship, before.authorship);
        } else if (transition === 'source')
          f.transport.dispatch({ changes: { from: 0, insert: 'x' } });
        else if (transition === 'selection')
          f.transport.dispatch({
            selection: EditorSelection.cursor(1),
            annotations: Transaction.addToHistory.of(false),
          });
        else if (transition === 'endings')
          f.transport.dispatch({
            ...exactReplacement(f.state(), 0, 4, 'a\nb'),
            annotations: Transaction.addToHistory.of(false),
          });
        else
          f.transport.dispatch({
            effects:
              transition === 'context'
                ? invalidateCommandContext.of(null)
                : installAuthorship.of(preparedAuthorship([], [{ from: 0, to: 1 }])),
            annotations: Transaction.addToHistory.of(false),
          });
      };
      try {
        if (transition === 'history') f.transport.dispatch({ changes: { from: 0, insert: 'x' } });
        if (phase === 'cleanup')
          Object.assign(f.transport, {
            plugin: () => ({
              hasCompositionInput: () => false,
              reserveCommand: () => ({ finish() {}, cancel: change }),
            }),
          });
        const pending = f.updates.captureDocument();
        if (phase === 'before-read') change();
        if (phase === 'between-read-write') {
          const measure = f.measurements.shift()!;
          const ready = measure.read(f.transport as unknown as Parameters<typeof measure.read>[0]);
          change();
          measure.write?.(ready, f.transport as unknown as Parameters<typeof measure.read>[0]);
        } else f.flush();
        assert.deepEqual(await pending, {
          status: 'rejected',
          reason: 'superseded',
        });
      } finally {
        f.dispose();
      }
    });
  }
}
