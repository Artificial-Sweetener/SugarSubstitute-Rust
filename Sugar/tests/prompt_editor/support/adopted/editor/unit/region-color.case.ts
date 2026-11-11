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
/** Independent actual Qt6.11.2 operands qualify prepared RGB hover semantics without native or CSS work. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareRegionColor } from '../../../../../../frontend/sugarprompt/editor/src/region-color.ts';

// Root's no-window QColor receipt reconstructs these exact8-bit normal RGBs before lighter135.
const cases = [
  { ordinal: 0, total: 3, normal: [79, 109, 245], hover: [158, 176, 255] },
  { ordinal: 1, total: 3, normal: [245, 114, 53], hover: [255, 170, 131] },
  { ordinal: 2, total: 3, normal: [184, 245, 53], hover: [216, 255, 131] },
  { ordinal: 1, total: 2, normal: [245, 210, 53], hover: [255, 232, 131] },
  { ordinal: 1, total: 4, normal: [245, 53, 184], hover: [255, 131, 216] },
  { ordinal: 2, total: 4, normal: [245, 210, 53], hover: [255, 232, 131] },
  { ordinal: 3, total: 4, normal: [53, 245, 114], hover: [131, 255, 170] },
  { ordinal: 1, total: 5, normal: [245, 53, 242], hover: [255, 131, 253] },
  { ordinal: 2, total: 5, normal: [245, 94, 53], hover: [255, 157, 131] },
  { ordinal: 3, total: 5, normal: [165, 245, 53], hover: [203, 255, 131] },
  { ordinal: 4, total: 5, normal: [53, 245, 171], hover: [131, 255, 207] },
] as const;
for (const value of cases)
  test(`region ordinal${value.ordinal} of${value.total} retains normalRGB and the actual Qt hover result`, () => {
    const prepared = prepareRegionColor(value.ordinal, value.total);
    assert.deepEqual(prepared, {
      normal: value.ordinal === 0 ? '#4f6df5' : `rgb(${value.normal.join(', ')})`,
      hover: `rgb(${value.hover.join(', ')})`,
    });
    assert.equal(Object.isFrozen(prepared), true);
  });
