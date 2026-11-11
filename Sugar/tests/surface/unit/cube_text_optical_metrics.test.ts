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
/** Prove cap-height alignment independently of descenders, font size, and line-box leading. */
import { expect, test } from '@jest/globals';
import {
  cubeCapBaseline,
  cubeCapLineOffset,
} from '../../../frontend/comfyui/ui/surface/CubeTextOpticalMetrics.js';

test('places the capital center on the requested center and corrects asymmetric font descent', () => {
  const metrics = {
    actualBoundingBoxAscent: 9,
    actualBoundingBoxDescent: 0,
    fontBoundingBoxAscent: 14,
    fontBoundingBoxDescent: 3,
  };
  const baseline = cubeCapBaseline(20, metrics, 16);
  expect(baseline - metrics.actualBoundingBoxAscent / 2).toBe(20);
  expect(cubeCapLineOffset(metrics)).toBe(-1);
  const lineCenter =
    (metrics.fontBoundingBoxAscent -
      metrics.fontBoundingBoxDescent -
      metrics.actualBoundingBoxAscent) /
    2;
  expect(lineCenter + cubeCapLineOffset(metrics)).toBe(0);
});

test('uses a finite typography fallback when a host cannot report ink metrics', () => {
  const metrics = {
    actualBoundingBoxAscent: NaN,
    actualBoundingBoxDescent: NaN,
    fontBoundingBoxAscent: NaN,
    fontBoundingBoxDescent: NaN,
  };
  expect(cubeCapBaseline(20, metrics, 16)).toBeCloseTo(25.6);
  expect(cubeCapLineOffset(metrics)).toBe(0);
});
