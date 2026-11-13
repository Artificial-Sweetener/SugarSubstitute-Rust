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
/** Literal viewport specimens prove above-pointer placement, margin clamping and finite malformed-geometry refusal. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { weightPreviewPlacement } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview-view.ts';

for (const specimen of [
  {
    point: { x: 100, y: 90 },
    expected: { left: 80, top: 66, width: 40, height: 20 },
  },
  {
    point: { x: -40, y: -100 },
    expected: { left: 16, top: 26, width: 40, height: 20 },
  },
  {
    point: { x: 400, y: 500 },
    expected: { left: 164, top: 94, width: 40, height: 20 },
  },
])
  test(`passive preview retains host margins at pointer x=${specimen.point.x}`, () => {
    assert.deepEqual(
      weightPreviewPlacement({ width: 40, height: 20 }, specimen.point, {
        left: 10,
        top: 20,
        width: 200,
        height: 100,
      }),
      specimen.expected,
    );
  });

test('oversized and malformed native metrics are bounded without viewport guesses', () => {
  assert.deepEqual(
    weightPreviewPlacement(
      { width: 900, height: 300 },
      { x: 0, y: 0 },
      { left: 10, top: 20, width: 100, height: 40 },
    ),
    { left: 16, top: 26, width: 88, height: 28 },
  );
  assert.equal(
    weightPreviewPlacement(
      { width: NaN, height: 20 },
      { x: 0, y: 0 },
      { left: 10, top: 20, width: 100, height: 40 },
    ),
    null,
  );
  assert.equal(
    weightPreviewPlacement(
      { width: 10, height: 20 },
      { x: 0, y: 0 },
      { left: 10, top: 20, width: 12, height: 40 },
    ),
    null,
  );
});
