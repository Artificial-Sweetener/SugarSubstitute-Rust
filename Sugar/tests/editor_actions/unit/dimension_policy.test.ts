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
/** Characterize Substitute's exact dimension pairing and anchor-preserving arithmetic. */
import { expect, test } from '@jest/globals';
import {
  inferDimensionPairs,
  ratioDimension,
  canonicalDimensions,
} from '../../../frontend/comfyui/ui/editor_actions/domain/DimensionPolicy.js';

test('pairs only exact keys with the same stem, in discovery order', () => {
  expect(
    inferDimensionPairs([
      'target_height',
      'width',
      'source_width',
      'height',
      'target_width',
      'Width',
      '_height',
      'source_height',
    ]),
  ).toEqual([
    { stem: 'target', width: 'target_width', height: 'target_height' },
    { stem: '', width: 'width', height: 'height' },
    { stem: 'source', width: 'source_width', height: 'source_height' },
  ]);
  expect(inferDimensionPairs(['source_width', 'target_height'])).toEqual([]);
});
test('preserves the anchor and rounds ties to even like the reference', () => {
  expect(ratioDimension(1024, 'width', 16, 9)).toBe(576);
  expect(ratioDimension(768, 'height', 3, 2)).toBe(1152);
  expect(ratioDimension(5, 'width', 2, 1)).toBe(2);
  expect(ratioDimension(7, 'width', 2, 1)).toBe(4);
  expect(ratioDimension(8.166666666666666, 'width', 21, 9)).toBe(4);
  expect(ratioDimension(NaN, 'width', 2, 1)).toBeNull();
  expect(canonicalDimensions(1344, 960)).toEqual({ short_edge: 960, long_edge: 1344 });
  expect(canonicalDimensions(0, 10)).toBeNull();
});
