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
/** Genuine mixed-carrier reorder commands conserve directional source selection and one native history event. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redo, undo, undoDepth } from '@codemirror/commands';
import {
  hideReorderChip,
  moveReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState, transactionState } from './reorder-fixture.ts';
import { prepareReorderCommit } from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

test('mixed comma and adjacent LoRA carriers move through actual commit, undo and redo', () => {
  const source = 'alpha, <lora:real:0.70>    <lora:missing:0.50>, tail',
    expected = 'alpha, <lora:missing:0.50>, tail, <lora:real:0.70>',
    from = 'alpha, '.length,
    to = from + '<lora:real:0.70>'.length,
    { state } = reorderState(source, to, from),
    capture = captureReorder(state);
  const hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 1);
  assert.ok(hidden);
  assert.equal(readExactSource(state) === source, true);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [to, from]);
  assert.equal(undoDepth(state), 0);
  const moved = moveReorderChip(capture.prepared, capture.prepared.initial, 1, {
    kind: 'row',
    row: 0,
    insertion: 3,
  });
  assert.ok(moved);
  const result = prepareReorderCommit(state, capture, moved);
  assert.equal(result.status, 'ready');
  assert.equal(result.edit.transactions.length, 1);
  const native = transactionState(state);
  native.dispatch(result.edit.transactions[0]!);
  assert.equal(readExactSource(native.state) === expected, true);
  assert.deepEqual(
    [native.state.selection.main.anchor, native.state.selection.main.head],
    [expected.length, expected.length - (to - from)],
  );
  assert.equal(undoDepth(native.state), 1);
  assert.equal(undo(native), true);
  assert.equal(readExactSource(native.state) === source, true);
  assert.deepEqual(
    [native.state.selection.main.anchor, native.state.selection.main.head],
    [to, from],
  );
  assert.equal(redo(native), true);
  assert.equal(readExactSource(native.state) === expected, true);
});
