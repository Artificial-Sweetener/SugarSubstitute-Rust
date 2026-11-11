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
/** Native transaction/history proof for original newline spelling, including grouped history effects. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text, Transaction } from '@codemirror/state';
import { history, redo, undo } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactReplacement,
  exactSourceExtension,
  lineEndings,
  prepareExactReplacementRange,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Construct a real CodeMirror state with no DOM or mocked history. */
function stateFor(source: string): EditorState {
  return EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    extensions: [exactSourceExtension(source), history()],
  });
}

test('mixed endings survive insertion, cross-line deletion and native undo/redo', () => {
  let state = stateFor('first\r\nsecond\nthird\rfourth');
  const before = readExactSource(state);
  state = state.update(exactReplacement(state, 7, 13, 'middle\r\nnew')).state;
  assert.equal(readExactSource(state), 'first\r\nmiddle\r\nnew\nthird\rfourth');
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), before);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), 'first\r\nmiddle\r\nnew\nthird\rfourth');
  state = state.update(exactReplacement(state, 5, 19, ' ')).state;
  assert.equal(readExactSource(state), 'first third\rfourth');
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'first\r\nmiddle\r\nnew\nthird\rfourth');
});

test('metadata-only line-ending replacement has native history', () => {
  let state = stateFor('a\nb');
  state = state.update(exactReplacement(state, 1, 2, '\r\n')).state;
  assert.equal(readExactSource(state), 'a\r\nb');
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'a\nb');
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'a\r\nb');
});

test('cross-line replacement restores identical forward newline metadata during undo and redo', () => {
  for (const ending of ['\r', '\r\n']) {
    const before = `old${ending}tail`,
      after = `new${ending}end`;
    let state = stateFor(before);
    state = state.update(exactReplacement(state, 0, before.length, after)).state;
    assert.equal(readExactSource(state), after);
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), before);
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    });
    assert.equal(readExactSource(state), after);
  }
});

test('grouped native typing/deletion restores original endings and directional selection', () => {
  let state = stateFor('a\r\n\rb');
  state = state.update({ selection: { anchor: 4, head: 0 } }).state;
  state = state.update({
    changes: { from: 2, to: 3 },
    userEvent: 'delete.backward',
    annotations: Transaction.time.of(100),
  }).state;
  state = state.update({
    changes: { from: 1, to: 2 },
    userEvent: 'delete.backward',
    annotations: Transaction.time.of(101),
  }).state;
  assert.equal(readExactSource(state), 'ab');
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'a\r\n\rb');
  assert.equal(state.selection.main.anchor, 4);
  assert.equal(state.selection.main.head, 0);
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'ab');
});

test('disjoint simultaneous edits retain untouched endings and reject malformed boundaries', () => {
  let state = stateFor('a\r\nb\nc\rd');
  state = state.update({
    changes: [
      { from: 0, insert: 'X\n' },
      { from: 6, to: 7, insert: 'Y' },
    ],
  }).state;
  assert.equal(readExactSource(state), 'X\na\r\nb\nc\rY');
  assert.throws(() => exactReplacement(state, 4, 5, ''), /inside CRLF/u);
  assert.throws(() => exactReplacement(state, 5, 2, ''), RangeError);
});

test('candidate alternatives share actual unchanged newline metadata and preserve exact native history', () => {
  const source = 'prefix\r\n<lora:mod:0.80:0.60>\rtail\nend';
  let state = stateFor(source).update({ selection: { anchor: 14 } }).state;
  const slot = prepareExactReplacementRange(state, 8, 28);
  const endings = state.field(lineEndings);
  const first = slot.replace('<lora:first:0.80:0.60>');
  const second = slot.replace('<lora:second:0.80:0.60>');
  assert.equal(first.effects[0]!.value, endings);
  assert.equal(second.effects[0]!.value, endings);
  state = state.update(second).state;
  assert.equal(readExactSource(state), 'prefix\r\n<lora:second:0.80:0.60>\rtail\nend');
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), source);
  assert.equal(state.selection.main.head, 14);
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), 'prefix\r\n<lora:second:0.80:0.60>\rtail\nend');
});

test('prepared replacement slots splice changed spellings independently and validate exact boundaries at preparation', () => {
  const state = stateFor('a\r\nb\rc\nd');
  const slot = prepareExactReplacementRange(state, 1, 4);
  const changed = slot.replace('\nX\r\nY');
  assert.notEqual(changed.effects[0]!.value, state.field(lineEndings));
  assert.equal(readExactSource(state.update(changed).state), 'a\nX\r\nY\rc\nd');
  assert.equal(readExactSource(state.update(slot.replace('\r\nB')).state), 'a\r\nB\rc\nd');
  assert.throws(() => prepareExactReplacementRange(state, 2, 3), /inside CRLF/u);
  assert.throws(() => prepareExactReplacementRange(state, 4, 1), RangeError);
});
