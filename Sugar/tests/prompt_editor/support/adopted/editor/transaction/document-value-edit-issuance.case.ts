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
/** Actual native command issuance rejects retired or mutated payloads and preserves genuine reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState } from '@codemirror/state';
import { undo, undoDepth } from '@codemirror/commands';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  prepareDocumentValueEdit,
  currentDocumentValueEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-edit-issuance.ts';
import {
  valueState,
  valueContext,
  valueTarget,
  decodedEdit,
  acceptedEdit,
  editedState,
} from './document-value-fixture.ts';
import { StateEffect } from '@codemirror/state';

test('publication fence rejects source/selection/context changes after preparation and accepts freshly prepared followups', () => {
  const { state } = valueState('h\na'),
    context = valueContext(state),
    target = valueTarget(state, context, 2);
  const edit = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'b'));
  assert.equal(currentDocumentValueEdit(state, context, edit), null);
  for (const next of [
    state.update(exactReplacement(state, 2, 3, 'c')).state,
    state.update({ selection: { anchor: 3 } }).state,
    state.update({ effects: invalidateCommandContext.of(null) }).state,
  ]) {
    const fresh = valueContext(next);
    assert.notEqual(currentDocumentValueEdit(next, fresh, edit), null);
    const followup = acceptedEdit(next, decodedEdit(fresh, valueTarget(next, fresh, 2), 0, 1, 'd'));
    assert.equal(currentDocumentValueEdit(next, fresh, followup), null);
    assert.equal(readExactSource(editedState(followup)) === 'h\nd', true);
  }
});

test('unexpected canonical/native invariant errors retain their original identity; no source or cause translation occurs', () => {
  const original = new Error('synthetic-invariant');
  let { state } = valueState('h\na');
  state = state.update({
    effects: StateEffect.appendConfig.of(
      EditorState.transactionFilter.of(() => {
        throw original;
      }),
    ),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 2);
  assert.throws(
    () => prepareDocumentValueEdit(state, decodedEdit(context, target, 0, 1, 'b')),
    (error: unknown) => error === original,
  );
  const ordinary = valueState('h\na').state,
    ready = valueContext(ordinary);
  assert.equal(
    acceptedEdit(ordinary, decodedEdit(ready, valueTarget(ordinary, ready, 2), 0, 1, 'b'))
      .transactions.length,
    1,
  );
});

test('prompt adaptation retains full-source newline edits and command epochs do not revive after undo', () => {
  const { state } = valueState('a\r\n😀', 'prompt'),
    context = valueContext(state),
    target = valueTarget(state, context, 0);
  const edit = acceptedEdit(state, decodedEdit(context, target, 0, 1, 'b\rc', 3, 0));
  let next = editedState(edit);
  assert.equal(readExactSource(next) === 'b\rc\r\n😀', true);
  assert.deepEqual([next.selection.main.anchor, next.selection.main.head], [3, 0]);
  assert.equal(
    undo({
      state: next,
      dispatch: (transaction) => {
        next = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(next) === 'a\r\n😀', true);
  assert.equal(currentDocumentValueEdit(next, valueContext(next), edit), 'stale-source');
  const fresh = valueContext(next);
  assert.equal(
    acceptedEdit(next, decodedEdit(fresh, valueTarget(next, fresh, 0), 0, 1, 'z')).transactions
      .length,
    1,
  );
});

test('nested native payload mutation retires command issuance even after contents are restored', () => {
  for (const mutation of ['endings', 'ranges', 'goal', 'association'] as const) {
    const { state } = valueState('h\r\n"{a}"'),
      context = valueContext(state),
      target = valueTarget(state, context, 3);
    const edit = acceptedEdit(state, decodedEdit(context, target, 0, 3, '{b}', 3, 0));
    const transaction = edit.transactions[0]!,
      next = transaction.state,
      selection = transaction.selection!;
    assert.equal(currentDocumentValueEdit(state, context, edit), null);
    let restore: () => void;
    if (mutation === 'endings') {
      const endings: unknown = transaction.effects[0]!.value;
      assert.ok(Array.isArray(endings));
      const original: unknown = endings[0];
      assert.equal(Reflect.set(endings, '0', '\n'), true);
      assert.equal(readExactSource(next) === edit.preparedValues.source, false);
      restore = () => void Reflect.set(endings, '0', original);
    } else if (mutation === 'ranges') {
      const original = selection.ranges[0]!;
      assert.equal(Reflect.set(selection.ranges, '0', EditorSelection.cursor(0)), true);
      assert.deepEqual([next.selection.main.anchor, next.selection.main.head], [0, 0]);
      restore = () => void Reflect.set(selection.ranges, '0', original);
    } else if (mutation === 'goal') {
      const range = selection.main,
        original = range.goalColumn;
      assert.equal(Reflect.set(range, 'goalColumn', 17), true);
      restore = () => void Reflect.set(range, 'goalColumn', original);
    } else {
      const original = selection.ranges[0]!;
      assert.equal(
        Reflect.set(
          selection.ranges,
          '0',
          EditorSelection.range(
            original.anchor,
            original.head,
            original.goalColumn,
            original.bidiLevel ?? undefined,
            original.assoc === 1 ? -1 : 1,
          ),
        ),
        true,
      );
      restore = () => void Reflect.set(selection.ranges, '0', original);
    }
    assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
    restore();
    assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
    const followup = acceptedEdit(state, decodedEdit(context, target, 0, 3, '{c}', 3, 0));
    assert.equal(currentDocumentValueEdit(state, context, followup), null);
    assert.equal(readExactSource(editedState(followup)) === 'h\r\n"{c}"', true);
    assert.deepEqual(
      [editedState(followup).selection.main.anchor, editedState(followup).selection.main.head],
      [6, 3],
    );
    assert.equal(undoDepth(editedState(followup)), 1);
  }
});

test('producer refuses actual filtered source or selection that disagrees with canonical edit output', () => {
  for (const result of ['source', 'selection'] as const) {
    const initial = valueState('h\na').state;
    const state = initial.update({
      effects: StateEffect.appendConfig.of(
        EditorState.transactionFilter.of((transaction) => [
          transaction,
          result === 'source'
            ? { changes: { from: 2, to: 3, insert: 'c' }, sequential: true }
            : { selection: EditorSelection.single(0) },
        ]),
      ),
    }).state;
    const context = valueContext(state);
    assert.throws(
      () => acceptedEdit(state, decodedEdit(context, valueTarget(state, context, 2), 0, 1, 'b')),
      /decoded-edit-actual-result-mismatch/u,
    );
    assert.equal(readExactSource(state) === 'h\na', true);
    assert.equal(undoDepth(state), 0);
  }
});

test('missing payload slots and undefined goal collisions permanently retire genuine native command issuance', () => {
  for (const mutation of [
    'ending-hole',
    'range-hole',
    'effect-hole',
    'goal-NaN',
    'goal-null',
  ] as const) {
    const { state } = valueState('h\r\n"{a}"'),
      context = valueContext(state),
      target = valueTarget(state, context, 3),
      edit = acceptedEdit(state, decodedEdit(context, target, 0, 3, '{b}', 3, 0)),
      transaction = edit.transactions[0]!;
    assert.equal(currentDocumentValueEdit(state, context, edit), null);
    assert.equal(transaction.state.selection.main.goalColumn, undefined);
    let restore: () => void;
    if (mutation.startsWith('goal-')) {
      const range = transaction.selection!.main;
      assert.equal(Reflect.set(range, 'goalColumn', mutation === 'goal-NaN' ? NaN : null), true);
      restore = () => void Reflect.set(range, 'goalColumn', undefined);
    } else {
      const payload: unknown =
        mutation === 'ending-hole'
          ? transaction.effects[0]!.value
          : mutation === 'range-hole'
            ? transaction.selection!.ranges
            : transaction.effects;
      assert.ok(Array.isArray(payload));
      const original: unknown = payload[0],
        length = payload.length;
      assert.equal(Reflect.deleteProperty(payload, '0'), true);
      assert.equal(payload.length, length);
      assert.equal(Object.hasOwn(payload, '0'), false);
      restore = () => void Reflect.set(payload, '0', original);
    }
    assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
    restore();
    assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
    const followup = acceptedEdit(state, decodedEdit(context, target, 0, 3, '{c}', 3, 0));
    assert.equal(currentDocumentValueEdit(state, context, followup), null);
    assert.equal(followup.transactions[0]!.state.selection.main.goalColumn, undefined);
    assert.equal(readExactSource(editedState(followup)) === 'h\r\n"{c}"', true);
    assert.equal(undoDepth(editedState(followup)), 1);
  }
});

test('genuine no-op keeps native vertical-navigation metadata and populated output under exact selection evidence', () => {
  const initial = valueState('h\r\n"{a}"').state;
  const state = initial.update({
    selection: EditorSelection.create([EditorSelection.range(6, 3, undefined, 2, -1)]),
  }).state;
  const context = valueContext(state),
    target = valueTarget(state, context, 3);
  const edit = acceptedEdit(state, decodedEdit(context, target, 0, 3, '{a}', 3, 0));
  assert.equal(edit.transactions.length, 0);
  assert.equal(edit.postValues, context.values);
  assert.equal(edit.semantics, context.semantics);
  assert.equal(editedState(edit).selection, state.selection);
  assert.equal(state.selection.main.goalColumn, undefined);
  assert.equal(state.selection.main.bidiLevel, 2);
  assert.equal(state.selection.main.assoc, -1);
  assert.equal(currentDocumentValueEdit(state, context, edit), null);
  assert.equal(Reflect.set(state.selection.main, 'goalColumn', 17), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
  assert.equal(Reflect.set(state.selection.main, 'goalColumn', undefined), true);
  assert.equal(currentDocumentValueEdit(state, context, edit), 'foreign');
});
