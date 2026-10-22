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
/** Real native state transactions prove exact-issued passive opacity ownership without changing source, selection or history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, EditorSelection, type EditorState } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { EditorView } from '@codemirror/view';
import { captureReorder, reorderState } from './reorder-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  setReorderMotionMask,
  releaseReorderMotionMask,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import {
  reorderPreview,
  reorderPreviewExtension,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
test('an admitted complete mask owns immutable chip IDs and stale releases cannot retire its replacement', () => {
  let state = reorderState('a, b, c', 2, 0).state;
  state = state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  state = publishInitial(state);
  const capture = state.field(reorderPreview).value!.capture,
    original = readExactSource(state),
    selection = state.selection,
    ids = [0, 2],
    first = {},
    second = {};
  state = state.update({
    effects: setReorderMotionMask.of({
      issue: first,
      scope: first,
      capture,
      ids,
    }),
  }).state;
  ids[0] = 1;
  assert.deepEqual(state.field(reorderPreview).mask?.ids, [0, 2]);
  assert.equal(state.field(reorderPreview).mask?.scope, first);
  assert.equal(readExactSource(state), original);
  assert.equal(state.selection, selection);
  assert.equal(undoDepth(state), 0);
  state = state.update({
    effects: setReorderMotionMask.of({
      issue: second,
      scope: first,
      capture,
      ids: [1],
    }),
  }).state;
  const accepted = state.field(reorderPreview).mask;
  assert.equal(accepted?.scope, first);
  state = state.update({ effects: releaseReorderMotionMask.of(first) }).state;
  assert.equal(state.field(reorderPreview).mask, accepted);
  state = state.update({ effects: releaseReorderMotionMask.of(second) }).state;
  assert.equal(state.field(reorderPreview).mask, null);
  assert.equal(readExactSource(state), original);
  assert.equal(state.selection, selection);
  assert.equal(undoDepth(state), 0);
});
test('independent native states with equal chip IDs emit disjoint exact-issued mask selectors and retire independently', () => {
  const issues = [{}, {}];
  const states = issues.map((issue) => {
    const base = publishInitial(
      reorderState('a, b, c').state.update({
        effects: StateEffect.appendConfig.of(reorderPreviewExtension),
      }).state,
    );
    return base.update({
      effects: setReorderMotionMask.of({
        issue,
        scope: issue,
        capture: base.field(reorderPreview).value!.capture,
        ids: [0, 1],
      }),
    }).state;
  });
  const attributes = states.map((state) =>
    state
      .facet(EditorView.editorAttributes)
      .flatMap((source) => (typeof source === 'function' ? [] : [source]))
      .find((source) => source['data-reorder-motion-issue']),
  );
  assert.equal(attributes[0]?.['data-reorder-motion'], '0 1');
  assert.equal(attributes[1]?.['data-reorder-motion'], '0 1');
  assert.equal(attributes[0]?.['data-reorder-motion-issue'], reorderPaintIssueName(issues[0]!));
  assert.notEqual(
    attributes[0]?.['data-reorder-motion-issue'],
    attributes[1]?.['data-reorder-motion-issue'],
  );
  assert.notEqual(
    attributes[0]?.['data-reorder-motion-scope'],
    attributes[1]?.['data-reorder-motion-scope'],
  );
  const retained = states[1]!.field(reorderPreview).mask;
  assert.equal(
    states[0]!
      .update({ effects: releaseReorderMotionMask.of(issues[0]!) })
      .state.field(reorderPreview).mask,
    null,
  );
  assert.equal(
    states[1]!
      .update({ effects: releaseReorderMotionMask.of(issues[0]!) })
      .state.field(reorderPreview).mask,
    retained,
  );
});
test('canonical source and directional selection freshness retire passive opacity immediately', () => {
  for (const change of ['source', 'selection'] as const) {
    let state = reorderState('a, b, c').state;
    state = state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state;
    state = publishInitial(state);
    const capture = state.field(reorderPreview).value!.capture;
    state = state.update({
      effects: setReorderMotionMask.of({
        issue: {},
        scope: {},
        capture,
        ids: [0],
      }),
    }).state;
    state = state.update(
      change === 'source'
        ? { changes: { from: 0, insert: 'x' } }
        : { selection: EditorSelection.range(3, 1) },
    ).state;
    assert.equal(state.field(reorderPreview).mask, null);
    assert.equal(undoDepth(state), change === 'source' ? 1 : 0);
  }
});

/** Publish the genuine current initial operand before admitting passive visibility. */
function publishInitial(state: EditorState): EditorState {
  const capture = captureReorder(state);
  const preview = prepareReorderPreview(
    { token: {}, capture, activeChip: 0, order: capture.prepared.initial },
    'complete',
  );
  assert.ok(preview);
  return state.update({ effects: installReorderPreview.of(preview) }).state;
}

test('preview retirement clears capture and opacity in the same native transaction', () => {
  let state = publishInitial(
    reorderState('a, b, c').state.update({
      effects: StateEffect.appendConfig.of(reorderPreviewExtension),
    }).state,
  );
  const capture = state.field(reorderPreview).value!.capture;
  state = state.update({
    effects: setReorderMotionMask.of({
      issue: {},
      scope: {},
      capture,
      ids: [0],
    }),
  }).state;
  assert.ok(state.field(reorderPreview).mask);
  state = state.update({ effects: installReorderPreview.of(null) }).state;
  assert.equal(state.field(reorderPreview).value, null);
  assert.equal(state.field(reorderPreview).mask, null);
  assert.equal(undoDepth(state), 0);
});
