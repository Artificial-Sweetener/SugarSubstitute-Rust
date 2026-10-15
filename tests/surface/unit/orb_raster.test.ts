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
/** Prove display-density sizing and alpha-safe detail reduction for the composed orb. */
import { expect, test } from '@jest/globals';
import {
  orbPhysicalEdge,
  resampleOrbRaster,
} from '../../../frontend/comfyui/ui/surface/frame/OrbRaster.js';

test.each([
  [0.8, 37],
  [0.5, 23],
  [1, 46],
  [1.25, 58],
  [1.5, 69],
  [2, 92],
  [3, 138],
  [0, 46],
  [Number.NaN, 46],
])('density %s produces %s physical pixels', (density, edge) => {
  expect(orbPhysicalEdge(density)).toBe(edge);
});
test('flat color remains unchanged through reduction and source storage stays immutable', () => {
  const source = new Uint8ClampedArray(8 * 8 * 4);
  for (let offset = 0; offset < source.length; offset += 4) source.set([80, 120, 200, 128], offset);
  const result = resampleOrbRaster(source, 8, 2);
  expect([...result]).toEqual(Array.from({ length: 4 }, () => [80, 120, 200, 128]).flat());
  expect(resampleOrbRaster(source, 8, 8)).not.toBe(source);
});
test('a high-frequency checkerboard integrates rather than aliasing to either extreme', () => {
  const source = new Uint8ClampedArray(16 * 16 * 4);
  for (let y = 0; y < 16; y++)
    for (let x = 0; x < 16; x++) {
      const color = ((x + y) % 2) * 255;
      source.set([color, color, color, 255], (y * 16 + x) * 4);
    }
  const result = resampleOrbRaster(source, 16, 1);
  expect([...result]).toEqual([128, 128, 128, 255]);
});
test('transparent blue neighbors cannot introduce blue fringes around an opaque red edge', () => {
  const source = new Uint8ClampedArray([
    255, 0, 0, 255, 0, 0, 255, 0, 255, 0, 0, 255, 0, 0, 255, 0,
  ]);
  expect([...resampleOrbRaster(source, 2, 1)]).toEqual([255, 0, 0, 128]);
  expect([...resampleOrbRaster(new Uint8ClampedArray(16), 2, 1)]).toEqual([0, 0, 0, 0]);
});
test('malformed dimensions fail before allocating a display raster', () => {
  expect(() => resampleOrbRaster(new Uint8ClampedArray(4), 2, 1)).toThrow(RangeError);
  expect(() => resampleOrbRaster(new Uint8ClampedArray(4), 1, 0)).toThrow(RangeError);
});
