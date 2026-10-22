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
/** Actual issued candidates admit native deletion cursor resets while unrelated selection branches remain retired. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, type Transaction } from '@codemirror/state';
import { deleteGroupBackward, deleteGroupForward } from '@codemirror/commands';
import { completionState, sessionResult } from './completion-session-fixture.ts';
import {
  installCompletion,
  completionContinuation,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  followNativeCompletionContinuation,
  completionContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Prepare every command at the actual native cursor, including association and vertical-navigation intent. */
function fixture(forward = false) {
  const source = forward ? '<lora:real.pt:1>' : '🙂\r\nhan\rtail';
  const caret = forward ? 9 : 6;
  let state = completionState(source, caret);
  state = state.update({
    selection: EditorSelection.create([EditorSelection.cursor(caret, 1, 2, 18)]),
  }).state;
  const result = sessionResult(
    state,
    [{ name: 'hands', backend: 'hands.pt' }],
    forward ? 'lora' : 'tag',
  );
  state = state.update({ effects: installCompletion.of(result) }).state;
  const handle = completionContinuation(state);
  assert.ok(handle);
  return { state, result, handle, caret };
}

test('supported CodeMirror delete commands reset complete cursor flags without losing genuine acceptance', () => {
  for (const forward of [false, true]) {
    const f = fixture(true);
    let actual: Transaction | undefined;
    const command = forward ? deleteGroupForward : deleteGroupBackward;
    assert.equal(
      command({
        state: f.state,
        dispatch: (transaction) => {
          actual = transaction;
        },
      }),
      true,
    );
    assert.ok(actual);
    const next = followNativeCompletionContinuation(f.handle, actual);
    assert.equal(next.status, 'ready');
    if (next.status !== 'ready') continue;
    const accepted = completionContinuationTransaction(
      actual.state,
      next.value.continuation,
      f.result,
      0,
      'keyboard',
    );
    assert.equal(accepted.status, 'ready');
    if (accepted.status === 'ready') {
      assert.equal(readExactSource(accepted.value.state), '<lora:hands:1>');
      assert.equal(
        accepted.value.newSelection,
        f.result.options[0]!.acceptance.keyboard.edit.transactions[0]!.newSelection,
      );
      assert.equal(
        accepted.value.effects[0],
        f.result.options[0]!.acceptance.keyboard.edit.transactions[0]!.effects[0],
      );
    }
  }
});

test("SugarPrompt adjacent deletion's default cursor reset retains the full prefix acceptance and exact endings", () => {
  const f = fixture();
  const transaction = f.state.update({
    changes: { from: 5, to: 6 },
    selection: { anchor: 5 },
    userEvent: 'delete.backward',
  });
  const next = followNativeCompletionContinuation(f.handle, transaction);
  assert.equal(next.status, 'ready');
  if (next.status !== 'ready') return;
  const accepted = completionContinuationTransaction(
    transaction.state,
    next.value.continuation,
    f.result,
    0,
    'pointer',
  );
  assert.equal(accepted.status, 'ready');
  if (accepted.status === 'ready')
    assert.equal(readExactSource(accepted.value.state), '🙂\r\nhands\rtail');
});

test('unrelated, wrong-direction and altered complete selection branches reject without retiring a valid sibling', () => {
  for (const invalid of [
    'interior',
    'direction',
    'insert',
    'bidi',
    'goal',
    'nonempty',
    'ranges',
    'main',
  ] as const) {
    const f = fixture();
    const selection =
      invalid === 'bidi'
        ? EditorSelection.create([EditorSelection.cursor(5, 0, 3)])
        : invalid === 'goal'
          ? EditorSelection.create([EditorSelection.cursor(5, 0, undefined, 8)])
          : invalid === 'nonempty'
            ? EditorSelection.single(4, 5)
            : invalid === 'ranges' || invalid === 'main'
              ? EditorSelection.create(
                  [EditorSelection.cursor(0), EditorSelection.cursor(5)],
                  invalid === 'main' ? 0 : 1,
                )
              : EditorSelection.single(5);
    const transaction = f.state.update({
      changes:
        invalid === 'interior'
          ? { from: 4, to: 5 }
          : { from: 5, to: 6, insert: invalid === 'insert' ? 'x' : '' },
      selection,
      userEvent: invalid === 'direction' ? 'delete.forward' : 'delete.backward',
    });
    assert.equal(
      followNativeCompletionContinuation(f.handle, transaction).status,
      'retired',
      invalid,
    );
    const valid = f.state.update({
      changes: { from: 5, to: 6 },
      selection: { anchor: 5 },
      userEvent: 'delete.backward',
    });
    assert.equal(followNativeCompletionContinuation(f.handle, valid).status, 'ready', invalid);
  }
});

test('source-neutral cursor flag changes remain exact and cannot borrow deletion authority', () => {
  const f = fixture();
  const transaction = f.state.update({
    selection: EditorSelection.single(f.caret),
  });
  assert.equal(followNativeCompletionContinuation(f.handle, transaction).status, 'retired');
  const valid = f.state.update({});
  assert.equal(followNativeCompletionContinuation(f.handle, valid).status, 'ready');
});
