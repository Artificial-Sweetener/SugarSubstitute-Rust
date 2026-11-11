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
/** Prove one neutral wash preserves composition context and effective monochrome coverage. */
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { maskPreviewPixels } from '../../../frontend/comfyui/ui/input_canvas/presentation/MaskPreviewWash.js';

test('blends black, gray and white mask values once over the original full-color composition', () => {
  const composition = encodeCanvasRaster(
    3,
    1,
    'rgba8',
    Uint8Array.from([80, 120, 160, 255, 80, 120, 160, 255, 80, 120, 160, 255]),
  );
  const mask = encodeCanvasRaster(3, 1, 'coverage8', Uint8Array.from([0, 128, 255]));
  expect([...maskPreviewPixels(mask, composition)]).toEqual([
    40, 60, 80, 255, 104, 124, 144, 255, 168, 188, 208, 255,
  ]);
});

test('shows coverage over transparent composition pixels without coloring the wash by channel name', () => {
  const composition = encodeCanvasRaster(
    2,
    1,
    'rgba8',
    Uint8Array.from([80, 120, 160, 0, 80, 120, 160, 0]),
  );
  const mask = encodeCanvasRaster(2, 1, 'coverage8', Uint8Array.from([0, 255]));
  expect([...maskPreviewPixels(mask, composition)]).toEqual([0, 0, 0, 128, 255, 255, 255, 128]);
});

test('fits a differently sized effective mask into the full composition frame with soft interpolation', () => {
  const composition = encodeCanvasRaster(
    3,
    1,
    'rgba8',
    Uint8Array.from([100, 100, 100, 255, 100, 100, 100, 255, 100, 100, 100, 255]),
  );
  const mask = encodeCanvasRaster(2, 1, 'coverage8', Uint8Array.from([0, 255]));
  expect([...maskPreviewPixels(mask, composition)]).toEqual([
    50, 50, 50, 255, 114, 114, 114, 255, 178, 178, 178, 255,
  ]);
});

test('rejects an uninterpreted color source instead of guessing its mask channel', () => {
  const image = encodeCanvasRaster(1, 1, 'rgba8', Uint8Array.from([10, 20, 30, 255]));
  expect(() => maskPreviewPixels(image, image)).toThrow('effective monochrome mask');
});
