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
/** Real reorder issuance conserves canonical boundaries, directional selection and native history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState, Transaction, type SelectionRange } from '@codemirror/state';
import { undo, redo, undoDepth, isolateHistory } from '@codemirror/commands';
import {
  moveReorderChip,
  hideReorderChip,
  dropReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState, transactionState } from './reorder-fixture.ts';
import {
  prepareReorderCommit,
  prepareNativeReorder,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { currentDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  semanticStructures,
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

test('one actual isolated reorder maps reverse selection and preserves exact history', () => {
  const { state } = reorderState('a, b, c', 1, 0),
    capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!;
  const result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  const tx = result.edit.transactions[0]!;
  assert.equal(currentDocumentValueEdit(state, capture.context, result.edit), null);
  assert.equal(readExactSource(tx.state) === 'b, c, a', true);
  assert.deepEqual([tx.state.selection.main.anchor, tx.state.selection.main.head], [7, 6]);
  assert.equal(tx.annotation(Transaction.userEvent), 'input.reorder');
  assert.equal(tx.annotation(isolateHistory), 'full');
  assert.equal(tx.scrollIntoView, true);
  const native = transactionState(state);
  native.dispatch(tx);
  assert.equal(undoDepth(native.state), 1);
  assert.equal(undo(native), true);
  assert.equal(readExactSource(native.state) === 'a, b, c', true);
  assert.deepEqual([native.state.selection.main.anchor, native.state.selection.main.head], [1, 0]);
  assert.equal(redo(native), true);
  assert.equal(readExactSource(native.state) === 'b, c, a', true);
});

test('unchanged order reuses populated genuine output and adds no source/selection/history transaction', () => {
  const { state } = reorderState('a\r\n, b', 1, 0),
    capture = captureReorder(state),
    result = prepareReorderCommit(state, capture, capture.prepared.initial);
  assert.equal(result.status, 'unchanged');
  assert.equal(result.edit.transactions.length, 0);
  assert.equal(result.edit.postValues, capture.context.values);
  assert.equal(result.edit.semantics, capture.context.semantics);
  assert.equal(result.edit.startState.selection, state.selection);
});

test('all in-value native directional ranges map through their own genuine chip identity', () => {
  let { state } = reorderState('a, b, c');
  state = state.update({
    selection: EditorSelection.create(
      [EditorSelection.range(1, 0), EditorSelection.range(4, 3)],
      1,
    ),
  }).state;
  const capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  const selected = result.edit.transactions[0]!.state.selection;
  assert.deepEqual(
    selected.ranges.map((r) => [r.anchor, r.head]),
    [
      [1, 0],
      [7, 6],
    ],
  );
  assert.deepEqual([selected.main.anchor, selected.main.head], [1, 0]);
});

test('quoted CSV replaces only the actual value while conserving header sibling and CRLF', () => {
  const { state } = reorderState('h,k\r\n"a, b",keep', 7, 7, 'wildcard-csv'),
    capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(readExactSource(result.edit.transactions[0]!.state) === 'h,k\r\n"b, a",keep', true);
  const semantic = state.field(semanticStructures).native!;
  assert.equal(
    prepareNativeReorder(
      state,
      {
        values: semantic.nativeValues,
        semantics: semantic.documentSemantics,
        enabled: true,
        composing: false,
      },
      { from: 0, to: 0 },
    ),
    null,
  );
});

test('scene headings remain actual chips and same-value regional crossing preserves empty partitions', () => {
  const { state } = reorderState('#Scene\na, b');
  const capture = captureReorder(state);
  assert.equal(
    capture.prepared.chips.some((c) => c.text === '#Scene'),
    true,
  );
  const headingOrder = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    heading = prepareReorderCommit(state, capture, headingOrder);
  assert.equal(heading.status, 'ready');
  assert.equal(readExactSource(heading.edit.transactions[0]!.state) === '\na, #Scene, b', true);
  const region = reorderState('a\n[SEP|Face]\nb', 0),
    regional = captureReorder(region.state),
    moved = moveReorderChip(regional.prepared, regional.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    });
  assert.notEqual(moved, null);
  const result = prepareReorderCommit(region.state, regional, moved!);
  assert.equal(result.status, 'ready');
  assert.equal(result.edit.postValues.scopes.scopes[0]!.document.regions.partitions.length, 2);
});

test('selection/source/permission changes and foreign captures cannot renew genuine command authority', () => {
  const fixture = reorderState('a, b'),
    capture = captureReorder(fixture.state);
  const selected = fixture.state.update({ selection: { anchor: 1 } }).state;
  assert.equal(
    prepareReorderCommit(selected, capture, capture.prepared.initial).status,
    'rejected',
  );
  const changed = fixture.state.update({
    changes: { from: 0, insert: 'x' },
  }).state;
  assert.equal(prepareReorderCommit(changed, capture, capture.prepared.initial).status, 'rejected');
  const readonly = fixture.state.update({
    effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(
    prepareReorderCommit(readonly, capture, capture.prepared.initial).status,
    'rejected',
  );
  assert.equal(
    prepareReorderCommit(fixture.state, { ...capture }, capture.prepared.initial).status,
    'rejected',
  );
});

test('quoted comma and non-BMP chips retain exact CSV quote spelling and independent sibling storage', () => {
  const { state } = reorderState('h,k\r\n"""a,b"", 🦊",keep', 5, 5, 'wildcard-csv'),
    capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  assert.equal(
    readExactSource(result.edit.transactions[0]!.state) === 'h,k\r\n"🦊, ""a,b""",keep',
    true,
  );
});

test('seeded actual post values and semantics are reused by the genuine following reorder', () => {
  const { state } = reorderState('a, b, c'),
    capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
    result = prepareReorderCommit(state, capture, order);
  assert.equal(result.status, 'ready');
  const changed = result.edit.transactions[0]!,
    seed = prepareNativeStructure(changed.state, result.edit),
    next = changed.state.update({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: seed,
      }),
      annotations: Transaction.addToHistory.of(false),
    }).state;
  assert.equal(seed.nativeValues, result.edit.postValues);
  assert.equal(seed.documentSemantics, result.edit.semantics);
  const following = captureReorder(next),
    back = moveReorderChip(following.prepared, following.prepared.initial, 2, {
      kind: 'row',
      row: 0,
      insertion: 0,
    })!,
    restored = prepareReorderCommit(next, following, back);
  assert.equal(restored.status, 'ready');
  assert.equal(readExactSource(restored.edit.transactions[0]!.state) === 'a, b, c', true);
});

test('observed native range payload mutation permanently retires capture while a fresh genuine following capture works', () => {
  const { state } = reorderState('a, b', 1, 0),
    capture = captureReorder(state),
    ranges = state.selection.ranges as unknown as SelectionRange[],
    original = ranges[0]!;
  ranges[0] = EditorSelection.range(0, 0);
  assert.equal(prepareReorderCommit(state, capture, capture.prepared.initial).status, 'rejected');
  ranges[0] = original;
  assert.equal(prepareReorderCommit(state, capture, capture.prepared.initial).status, 'rejected');
  const fresh = captureReorder(state);
  assert.equal(prepareReorderCommit(state, fresh, fresh.prepared.initial).status, 'unchanged');
});

test('a directional selection crossing regional partitions restores the selected moved chip rather than refusing the scope', () => {
  const source = 'a\n[SEP|Face]\nb',
    { state } = reorderState(source, source.length, 0),
    capture = captureReorder(state),
    order = moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
      kind: 'row',
      row: 0,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, order),
    expected = '\n[SEP|Face]\nb, a';
  assert.equal(result.status, 'ready');
  const selected = result.edit.transactions[0]!.state;
  assert.equal(readExactSource(selected) === expected, true);
  assert.deepEqual(
    [selected.selection.main.anchor, selected.selection.main.head],
    [expected.length, expected.length - 1],
  );
});

test('hard-row movement preserves both original newline carriers before the destination row', () => {
  const { state } = reorderState('a\nb\nc', 2),
    capture = captureReorder(state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 1)!,
    moved = dropReorderChip(capture.prepared, hidden, 1, {
      kind: 'row',
      row: 1,
      insertion: 1,
    })!,
    result = prepareReorderCommit(state, capture, moved);
  assert.equal(result.status, 'ready');
  assert.equal(readExactSource(result.edit.transactions[0]!.state) === 'a\n\nc, b', true);
  assert.deepEqual(result.edit.transactions[0]!.state.selection.main.head, 6);
});
