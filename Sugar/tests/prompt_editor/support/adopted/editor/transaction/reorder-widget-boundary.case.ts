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
/** Actual native replacement boundaries retain structured storage, logical source, selection and history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction } from '@codemirror/state';
import { WidgetType } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import {
  hideReorderChip,
  serializeReorderPreview,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { captureReorder, reorderState } from './reorder-fixture.ts';

test('a partial structured value replaces only its canonical semantic interval inside native line flow', () => {
  const source = 'h,k\r\n"a, b",keep';
  let state = reorderState(source, 7, 7, 'wildcard-csv').state;
  state = state.update({
    effects: StateEffect.appendConfig.of(reorderPreviewExtension),
  }).state;
  const before = state,
    capture = captureReorder(state),
    range = capture.target.entry.semanticMapping!.logicalRange;
  assert.ok(range.from > state.doc.lineAt(range.from).from);
  assert.ok(range.to < state.doc.lineAt(range.to).to);
  const order = hideReorderChip(capture.prepared, capture.prepared.initial, capture.active)!;
  assert.ok(order);
  const output = serializeReorderPreview(capture.prepared, order)!;
  assert.ok(output);
  state = state.update({
    effects: installReorderPreview.of({
      token: {},
      capture,
      activeChip: capture.active,
      order,
      output,
      presentation: 'collision',
      scenes: null,
    }),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  const replacements: {
    from: number;
    to: number;
    block: boolean;
    widget: WidgetType;
  }[] = [];
  state.field(reorderPreview).decorations.between(0, state.doc.length, (from, to, value) => {
    assert.ok(value.spec.widget instanceof WidgetType);
    replacements.push({
      from,
      to,
      block: value.spec.block === true,
      widget: value.spec.widget,
    });
  });
  assert.equal(replacements.length, 1);
  assert.deepEqual(
    {
      from: replacements[0]!.from,
      to: replacements[0]!.to,
      block: replacements[0]!.block,
    },
    { ...range, block: false },
  );
  assert.equal(replacements[0]!.widget.lineBreaks, 0);
  assert.equal(state.doc, before.doc);
  assert.equal(state.selection.eq(before.selection), true);
  assert.equal(readExactSource(state), source);
  assert.equal(undoDepth(state), 0);
  state = state.update({
    effects: installReorderPreview.of(null),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  assert.equal(state.field(reorderPreview).decorations.size, 0);
  assert.equal(state.doc, before.doc);
  assert.equal(state.selection.eq(before.selection), true);
  assert.equal(undoDepth(state), 0);
});
