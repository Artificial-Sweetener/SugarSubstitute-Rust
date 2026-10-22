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
/** Canonical name loading maps real native history and exact endings without a competing prompt/history owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import { sharedRegionNameEdit } from '../../../../../../frontend/sugarprompt/editor/src/region-host-transaction.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { captureRegionNames } from '../../../../../../frontend/sugarprompt/services/src/regions/names.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

test('minimal ordinal synchronization maps directional selection and undo/redo through CRLF and Unicode names', () => {
  const source = '🦊 global\r\n[SEP|A]\rbody\n[SEP|B]\n';
  let state = projectionState(source);
  state = state.update(exactReplacement(state, source.length, source.length, 'tail')).state;
  state = state.update({
    selection: { anchor: state.doc.length, head: source.indexOf('body') - 1 },
  }).state;
  const transaction = state.update(
    sharedRegionNameEdit(
      state,
      prepareNativeStructure(state),
      captureRegionNames([
        { ordinal: 0, name: '背景 🦊' },
        { ordinal: 1, name: '' },
        { ordinal: 99, name: 'unused' },
      ]),
    ),
  );
  const changed: number[][] = [];
  transaction.changes.iterChangedRanges((from, to) => {
    changed.push([from, to]);
  });
  assert.deepEqual(changed, [
    [source.indexOf('A') - 1, source.indexOf('A')],
    [source.indexOf('[SEP|B]') - 1, source.indexOf('[SEP|B]') + 6],
  ]);
  state = transaction.state;
  const synchronized = source.replace('A', '背景 🦊').replace('[SEP|B]', '[SEP]');
  assert.equal(readExactSource(state) === synchronized + 'tail', true);
  assert.equal(undoDepth(state), 1);
  assert.equal(state.selection.main.anchor, state.doc.length);
  assert.equal(
    state.doc.sliceString(state.selection.main.head, state.selection.main.head + 4),
    'body',
  );
  const target = {
    get state() {
      return state;
    },
    dispatch(transaction: import('@codemirror/state').Transaction) {
      state = transaction.state;
    },
  };
  assert.equal(undo(target), true);
  assert.equal(readExactSource(state) === synchronized, true);
  assert.equal(redo(target), true);
  assert.equal(readExactSource(state) === synchronized + 'tail', true);
});
