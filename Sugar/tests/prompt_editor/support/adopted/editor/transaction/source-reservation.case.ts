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
/** Actual native freshness and exact-source transformations prove deferred host replacements without a browser. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, Text } from '@codemirror/state';
import { history, undoDepth } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactSourceExtension,
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { PreparedSourceEdits } from '../../../../../../frontend/sugarprompt/editor/src/prepared-edits.ts';

/** Build the actual exact-source/history owner; selection uses logical native UTF-16 coordinates. */
function stateFor(): EditorState {
  const source = '🙂\r\ncat';
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), editorFreshness, history()],
  });
}

test('reserved ranges survive selection changes, preserve exact newlines and consume one history transaction', () => {
  const owner = new PreparedSourceEdits(),
    state = stateFor();
  const result = owner.reserve(state, false, 4, 7);
  assert.equal(result.status, 'ready');
  if (result.status !== 'ready') throw new Error('Reservation blocked');
  assert.equal(JSON.stringify(result.command), '{"kind":"source-reservation"}');
  assert.equal(undoDepth(state), 0);
  const selected = state.update({ selection: { anchor: 0 } }).state;
  const edit = owner.takeReserved(selected, false, result.command, '<lora:real:1.00>');
  assert.equal(edit.status, 'ready');
  if (edit.status !== 'ready') throw new Error('Acceptance rejected');
  const accepted = selected.update(edit.transaction).state;
  assert.equal(readExactSource(accepted), '🙂\r\n<lora:real:1.00>');
  assert.equal(undoDepth(accepted), 1);
  assert.deepEqual(owner.takeReserved(accepted, false, result.command, 'x'), {
    status: 'rejected',
    reason: 'consumed-command',
  });
  assert.throws(() => owner.reserve(state, false, 3, 3), RangeError);
  owner.dispose();
});

test('reserved commands share source/context, permission, composition, foreign and teardown rejection', () => {
  const state = stateFor();
  const preparation = new PreparedSourceEdits();
  assert.deepEqual(preparation.reserve(state, true, 4, 7), {
    status: 'blocked',
    reason: 'composing',
  });
  const readOnly = state.update({
    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
  }).state;
  assert.deepEqual(preparation.reserve(readOnly, false, 4, 7), {
    status: 'blocked',
    reason: 'read-only',
  });
  preparation.dispose();
  assert.deepEqual(preparation.reserve(state, false, 4, 7), {
    status: 'blocked',
    reason: 'disposed',
  });
  for (const [changed, reason] of [
    [state.update(exactReplacement(state, 4, 7, 'dog')).state, 'stale-source'],
    [state.update({ effects: invalidateCommandContext.of(null) }).state, 'stale-context'],
    [
      state.update({
        effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
      }).state,
      'read-only',
    ],
  ] as const) {
    const owner = new PreparedSourceEdits(),
      reserved = owner.reserve(state, false, 4, 7);
    if (reserved.status !== 'ready') throw new Error('Reservation blocked');
    assert.deepEqual(owner.takeReserved(changed, false, reserved.command, 'x'), {
      status: 'rejected',
      reason,
    });
    owner.dispose();
  }
  const owner = new PreparedSourceEdits(),
    reserved = owner.reserve(state, false, 4, 7);
  if (reserved.status !== 'ready') throw new Error('Reservation blocked');
  assert.deepEqual(owner.takeReserved(state, true, reserved.command, 'x'), {
    status: 'rejected',
    reason: 'composing',
  });
  assert.deepEqual(new PreparedSourceEdits().takeReserved(state, false, reserved.command, 'x'), {
    status: 'rejected',
    reason: 'foreign-command',
  });
  assert.deepEqual(owner.takeReserved(state, false, reserved.command, 'cat'), {
    status: 'unchanged',
  });
  assert.equal(undoDepth(state), 0);
  const disposed = owner.reserve(state, false, 4, 7);
  if (disposed.status !== 'ready') throw new Error('Reservation blocked');
  owner.dispose();
  assert.deepEqual(owner.takeReserved(state, false, disposed.command, 'x'), {
    status: 'rejected',
    reason: 'disposed',
  });
});
