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
/** Actual issued field publications retain canonical representation within one finite capture. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorView } from '@codemirror/view';
import { StateEffect, Transaction } from '@codemirror/state';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  prepareReorderPreview,
  sameReorderOrder,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { setReorderMotionMask } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import type { ReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-contracts.ts';

/** Publish only the actual owner-issued immutable output through the real CM field. */
function fixture() {
  let state = reorderState('a, b, c').state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  const capture = captureReorder(state),
    token = {};
  return {
    capture,
    token,
    /** Observe exact source/selection before and after non-history presentation updates. */
    get state() {
      return state;
    },
    /** Current prepared output grants no source transaction or authored history. */
    publish(value: ReorderPreview | null) {
      state = state.update({
        effects: installReorderPreview.of(value),
        annotations: Transaction.addToHistory.of(false),
      }).state;
      return state.field(reorderPreview);
    },
    /** A genuine Core displacement enters the sticky canonical phase independently of pointer admission. */
    displaced() {
      const order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
        kind: 'row',
        row: 0,
        insertion: 2,
      });
      assert.ok(order);
      const value = prepareReorderPreview(
        { token, capture, activeChip: capture.active, order },
        'pointer',
      );
      assert.ok(value);
      return value;
    },
    /** Core's complete exact initial operand is unchanged across pointer and complete presentation. */
    prepared(presentation: 'complete' | 'pointer', nextToken = token, nextCapture = capture) {
      const value = prepareReorderPreview(
        {
          token: nextToken,
          capture: nextCapture,
          activeChip: nextCapture.active,
          order: nextCapture.prepared.initial,
        },
        presentation,
      );
      assert.ok(value);
      return value;
    },
  };
}

test('initial pointer marks retain native source layout until an actual order change', () => {
  const f = fixture(),
    initial = f.prepared('complete'),
    source = f.state.doc.toString(),
    selection = f.state.selection;
  assert.equal(f.publish(initial).native, true);
  const pointer = f.prepared('pointer');
  assert.equal(sameReorderOrder(pointer.order, initial.capture.prepared.initial), true);
  assert.equal(f.publish(pointer).native, true);
  const returned = f.prepared('complete');
  assert.equal(sameReorderOrder(returned.order, initial.capture.prepared.initial), true);
  assert.equal(returned.output.text, initial.output.text);
  assert.equal(f.publish(returned).native, true);
  const marked = f.publish(pointer);
  let heldMarks = 0,
    replacementWidgets = 0;
  marked.decorations.between(0, f.state.doc.length, (_from, _to, decoration) => {
    if (decoration.spec.class?.includes('sp-reorder-held')) heldMarks++;
    if (decoration.spec.widget) replacementWidgets++;
  });
  assert.deepEqual([heldMarks, replacementWidgets], [0, 0]);
  const attributes = f.state
    .facet(EditorView.editorAttributes)
    .filter((source) => typeof source !== 'function');
  assert.ok(
    attributes.some(
      (source) =>
        source['data-reorder-native-held'] === 'true' &&
        source['data-reorder-native-active'] === '0',
    ),
  );
  assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  assert.equal(f.state.doc.toString(), source);
  assert.equal(f.state.selection, selection);
});

test('a genuine moved order stays canonical across original pointer, complete and mask-only issues', () => {
  const f = fixture(),
    source = f.state.doc,
    selection = f.state.selection;
  assert.equal(f.publish(f.prepared('pointer')).native, true);
  assert.equal(f.publish(f.displaced()).native, false);
  assert.equal(f.publish(f.prepared('pointer')).native, false);
  assert.equal(f.publish(f.prepared('complete')).native, false);
  const state = f.state.update({
    effects: setReorderMotionMask.of({
      issue: {},
      scope: {},
      capture: f.capture,
      ids: [1],
    }),
  }).state;
  assert.equal(state.field(reorderPreview).native, false);
  assert.equal(state.field(reorderPreview).mask !== null, true);
  assert.equal(state.doc === source, true);
  assert.equal(state.selection === selection, true);
  assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
});
test('new session capture and retired publication reset native entry', () => {
  const f = fixture();
  f.publish(f.displaced());
  assert.equal(f.publish(f.prepared('complete', {})).native, true);
  f.publish(f.displaced());
  const capture = captureReorder(f.state);
  assert.notEqual(capture, f.capture);
  assert.equal(f.publish(f.prepared('complete', f.token, capture)).native, true);
  f.publish(f.displaced());
  assert.equal(f.publish(null).value, null);
  assert.equal(f.publish(f.prepared('complete')).native, true);
});

test('unissued replacement DTO cannot establish a genuine canonical phase', () => {
  const f = fixture(),
    pointer = f.prepared('pointer');
  assert.equal(f.publish({ ...pointer }).native, false);
  assert.equal(f.publish(f.prepared('complete')).native, true);
});
