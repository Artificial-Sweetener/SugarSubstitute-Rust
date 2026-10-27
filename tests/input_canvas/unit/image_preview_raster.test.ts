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
/** Preserve bounded display geometry, linear-light color and alpha when native masks need full resolution. */
import { imagePreviewRaster } from '../../../frontend/comfyui/ui/input_canvas/domain/ImagePreviewRaster.js';
import {
  decodeCanvasRaster,
  encodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';

test('preserves existing small and full-resolution rasters without changing their identity', () => {
  const source = encodeCanvasRaster(
    2,
    1,
    'rgba8',
    Uint8Array.from([10, 20, 30, 40, 50, 60, 70, 80]),
  );
  expect(imagePreviewRaster(source, 0)).toBe(source);
  expect(imagePreviewRaster(source, 2)).toBe(source);
});
test('averages opaque color in linear light instead of darkening the image context', () => {
  const source = encodeCanvasRaster(
    2,
    1,
    'rgba8',
    Uint8Array.from([0, 0, 0, 255, 255, 255, 255, 255]),
  );
  const result = imagePreviewRaster(source, 1);
  expect([result.width, result.height]).toEqual([1, 1]);
  expect([...decodeCanvasRaster(result)]).toEqual([188, 188, 188, 255]);
  expect([...decodeCanvasRaster(source)]).toEqual([0, 0, 0, 255, 255, 255, 255, 255]);
});
test('keeps transparent hidden color out of the resized image and retains fractional alpha', () => {
  const source = encodeCanvasRaster(2, 1, 'rgba8', Uint8Array.from([255, 0, 0, 255, 0, 0, 255, 0]));
  expect([...decodeCanvasRaster(imagePreviewRaster(source, 1))]).toEqual([255, 0, 0, 128]);
});
test('bounds both image edges and preserves fully transparent coverage', () => {
  const result = imagePreviewRaster(
    encodeCanvasRaster(6, 4, 'rgba8', new Uint8Array(6 * 4 * 4)),
    3,
  );
  expect([result.width, result.height]).toEqual([3, 2]);
  expect([...decodeCanvasRaster(result)].every((value) => value === 0)).toBe(true);
});
test('rejects invalid display sizes and coverage rasters at the image boundary', () => {
  const source = encodeCanvasRaster(2, 1, 'rgba8', new Uint8Array(8));
  for (const size of [-1, 1.5, 1025])
    expect(() => imagePreviewRaster(source, size)).toThrow('Invalid image preview size');
  expect(() =>
    imagePreviewRaster(encodeCanvasRaster(1, 1, 'coverage8', new Uint8Array(1)), 1),
  ).toThrow('image pixels');
});
