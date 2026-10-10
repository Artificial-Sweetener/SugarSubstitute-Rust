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
/** Prove neutral adaptation retains alpha, colored details and the original decoded artwork. */
import { expect, test } from '@jest/globals';
import {
  classifyCubeIconTone,
  invertNeutralCubeIconPixels,
} from '../../../frontend/comfyui/ui/core/CubeIconToneProfile.js';

test('classifies opaque neutral polarity while excluding transparent padding and colored artwork', () => {
  expect(classifyCubeIconTone(new Uint8ClampedArray([255, 255, 255, 255, 0, 0, 0, 0]))).toBe(
    'bright',
  );
  expect(classifyCubeIconTone(new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 0]))).toBe(
    'dark',
  );
  expect(classifyCubeIconTone(new Uint8ClampedArray([140, 140, 140, 255]))).toBe('original');
  expect(classifyCubeIconTone(new Uint8ClampedArray([255, 0, 0, 255]))).toBe('original');
  expect(classifyCubeIconTone(new Uint8ClampedArray([0, 0, 0, 0]))).toBe('original');
});

test('inverts neutral visible pixels without mutating input, alpha or colored details', () => {
  const pixels = new Uint8ClampedArray([
    255, 255, 255, 200, 24, 28, 32, 128, 255, 0, 0, 255, 255, 255, 255, 0,
  ]);
  const original = new Uint8ClampedArray(pixels);
  expect(invertNeutralCubeIconPixels(pixels)).toEqual(
    new Uint8ClampedArray([0, 0, 0, 200, 231, 227, 223, 128, 255, 0, 0, 255, 255, 255, 255, 0]),
  );
  expect(pixels).toEqual(original);
});

test('rejects malformed pixel samples instead of classifying incomplete channels', () => {
  const invalid = new Uint8ClampedArray([255, 255]);
  expect(() => classifyCubeIconTone(invalid)).toThrow('RGBA');
  expect(() => invertNeutralCubeIconPixels(invalid)).toThrow('RGBA');
});
