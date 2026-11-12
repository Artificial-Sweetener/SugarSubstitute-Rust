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
/** Large native changes preserve exact newline metadata and the existing CodeMirror history owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text } from '@codemirror/state';
import { redo, redoDepth, undo, undoDepth } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactReplacement,
  exactSourceExtension,
  lineEndings,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  historyTransactions,
  promptHistory,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';

/** Construct the real document, exact newline field and native history. */
function stateFor(source: string): EditorState {
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), promptHistory],
  });
}

/** Exercise a complete native undo/redo cycle without another source or history owner. */
function assertHistory(
  state: EditorState,
  before: string,
  after: string,
  priorSelection: readonly [number, number],
): EditorState {
  assert.equal(readExactSource(state), after);
  assert.deepEqual([undoDepth(state), redoDepth(state)], [1, 0]);
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = historyTransactions([transaction]).at(-1)!.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), before);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], priorSelection);
  assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 1]);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = historyTransactions([transaction]).at(-1)!.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), after);
  assert.deepEqual([undoDepth(state), redoDepth(state)], [1, 0]);
  return state;
}

test('one native paste of 250001 newlines retains surrounding spellings, caret and directional history', () => {
  const before = '🙂\r\nreplace\rtail\nlast';
  let state = stateFor(before);
  const priorSelection = [state.doc.length, 3] as const;
  state = state.update({
    selection: { anchor: priorSelection[0], head: priorSelection[1] },
  }).state;
  const inserted = 'x\n'.repeat(250001);
  const caret = 3 + inserted.length;
  state = state.update({
    changes: { from: 3, to: 10, insert: inserted },
    selection: { anchor: caret },
    userEvent: 'input.paste',
  }).state;
  const after = '🙂\r\n' + inserted + '\rtail\nlast';
  const endings = state.field(lineEndings);
  assert.equal(endings.length, 250004);
  assert.equal(endings[0], '\r\n');
  assert.equal(endings[250001], '\n');
  assert.deepEqual(endings.slice(-2), ['\r', '\n']);
  state = assertHistory(state, before, after, priorSelection);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [caret, caret]);
});

test('a large exact replacement keeps every CRLF, CR and LF through native undo and redo', () => {
  const before = 'start\r\nold\rend\nlast';
  let state = stateFor(before).update({
    selection: { anchor: 9, head: 6 },
  }).state;
  const inserted = 'x\r\n'.repeat(125000) + 'y\r'.repeat(125000) + 'z\n';
  state = state.update(exactReplacement(state, 7, 10, inserted)).state;
  const after = 'start\r\n' + inserted + '\rend\nlast';
  const endings = state.field(lineEndings);
  assert.equal(endings.length, 250004);
  assert.equal(endings[125000], '\r\n');
  assert.equal(endings[125001], '\r');
  assert.deepEqual(endings.slice(-3), ['\n', '\r', '\n']);
  assertHistory(state, before, after, [9, 6]);
});

test('disjoint changes combine a large insertion, newline replacement and final insertion in original document coordinates', () => {
  const before = 'a\r\nb\rc\nd';
  let state = stateFor(before).update({
    selection: { anchor: 7, head: 0 },
  }).state;
  const inserted = 'x\n'.repeat(250001);
  state = state.update({
    changes: [
      { from: 0, to: 1, insert: inserted },
      { from: 2, to: 4, insert: 'z\nw\n' },
      { from: 7, insert: '!\n' },
    ],
    userEvent: 'input.paste',
  }).state;
  const after = inserted + '\r\nz\nw\nc\nd!\n';
  const endings = state.field(lineEndings);
  assert.equal(endings.length, 250006);
  assert.deepEqual(endings.slice(250001), ['\r\n', '\n', '\n', '\n', '\n']);
  assertHistory(state, before, after, [7, 0]);
});
