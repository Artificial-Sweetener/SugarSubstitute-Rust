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
/** Prove masks reflect the current channel while the source image stays interpretation-neutral. */
import {
  interpretMaskSource,
  maskSourceImage,
} from '../../../frontend/comfyui/ui/input_canvas/domain/MaskSourcePixels.js';
import {
  decodeCanvasRaster,
  encodeCanvasRaster,
} from '../../../integrations/shared/CanvasRasterEncoding.js';

test('each image channel produces its real mask and alpha follows Comfy inversion', () => {
  const source = encodeCanvasRaster(
    2,
    1,
    'rgba8',
    Uint8Array.from([10, 20, 30, 0, 60, 90, 120, 200]),
  );
  expect(maskSourceImage(source)).toBe(source);
  for (const [channel, expected] of [
    ['red', [10, 60]],
    ['green', [20, 90]],
    ['blue', [30, 120]],
    ['alpha', [255, 55]],
  ] as const)
    expect([...decodeCanvasRaster(interpretMaskSource(source, channel))]).toEqual(expected);
  expect([...decodeCanvasRaster(source)]).toEqual([10, 20, 30, 0, 60, 90, 120, 200]);
  expect(() => interpretMaskSource(source, 'luminance')).toThrow('Unsupported');
});

test('selection uses one fixed image representation independent of channel settings', () => {
  const source = encodeCanvasRaster(4, 1, 'coverage8', Uint8Array.from([0, 64, 128, 255]));
  const image = maskSourceImage(source);
  expect([...decodeCanvasRaster(image)]).toEqual([
    0, 0, 0, 255, 64, 64, 64, 191, 128, 128, 128, 127, 255, 255, 255, 0,
  ]);
  for (const channel of ['red', 'green', 'blue', 'alpha'])
    expect([...decodeCanvasRaster(interpretMaskSource(image, channel))]).toEqual([0, 64, 128, 255]);
});
