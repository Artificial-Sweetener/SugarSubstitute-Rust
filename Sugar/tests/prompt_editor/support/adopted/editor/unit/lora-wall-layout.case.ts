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
/** Independent visual specimens prove navigation and bounded viewport lookup without browser geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareLoraWallLayout,
  loraWallVerticalIndex,
  loraWallVisibleRange,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-wall-layout.ts';

test('vertical navigation follows nearest integer centres and wraps; viewport ranges retain the actual full geometry', () => {
  const layout = prepareLoraWallLayout(Array<number>(135).fill(0.72), 560);
  assert.equal(layout.rows[0]!.to, 4);
  assert.equal(loraWallVerticalIndex(layout, 1, 1), 5);
  assert.equal(loraWallVerticalIndex(layout, 5, -1), 1);
  assert.equal(loraWallVerticalIndex(layout, 0, -1), 132);
  assert.equal(loraWallVerticalIndex(layout, 133, 1), 1);
  const tail = loraWallVisibleRange(layout, layout.height - 200, 200);
  assert.deepEqual(tail, { from: 128, to: 135 });
  assert.equal(layout.boxes[134]!.index, 134);
  const selected = layout.boxes[1];
  loraWallVisibleRange(layout, 0, 630);
  assert.equal(layout.boxes[1], selected);
});

test('bounded complete layout keeps extreme ratios finite and rejects unsupported bounds', () => {
  const layout = prepareLoraWallLayout(Array<number>(100000).fill(0.72), 560);
  assert.equal(layout.boxes.length, 100000);
  assert.equal(layout.rows.length, 25000);
  assert.ok(loraWallVisibleRange(layout, layout.height - 630, 630).from > 99960);
  for (const width of [-1, Infinity, 16385])
    assert.throws(() => prepareLoraWallLayout([1], width), RangeError);
  assert.ok(
    prepareLoraWallLayout([NaN, Infinity, -1, 99], 126).boxes.every(
      (box) => box.width >= 1 && box.height >= 1,
    ),
  );
});
