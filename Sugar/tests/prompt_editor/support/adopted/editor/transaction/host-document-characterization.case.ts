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
/** Characterize the existing exact-source owner with independently specified coordinates and history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { HistoryFixture } from './history-fixture.ts';

test('complete mixed-newline replacement uses logical end and preserves exact history', () => {
  const native = new HistoryFixture('old');
  native.update(exactReplacement(native.state, 0, 3, '🙂\r\nx\ry\n\ud800'));
  assert.deepEqual(native.snapshot(), {
    source: '🙂\r\nx\ry\n\ud800',
    anchor: 9,
    head: 9,
    undoDepth: 1,
    redoDepth: 0,
  });
  assert.equal(native.state.selection.main.head, 8);
  native.select(9, 2);
  assert.equal(native.snapshot().anchor, 9);
  assert.equal(native.snapshot().head, 2);
  assert.throws(() => native.select(3, 3), RangeError);
  assert.equal(native.restore('undo'), true);
  assert.equal(native.snapshot().source, 'old');
  assert.equal(native.restore('redo'), true);
  assert.equal(native.snapshot().source, '🙂\r\nx\ry\n\ud800');
});
