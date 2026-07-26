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
/** Verify focused Cube port-label and association-leader primitives. */

import {
  elideCubePortLabel,
  resolveCubeOutputLeader,
} from '../../frontend/comfyui/ui/surface/CubeCanvasPortRenderer.js';
import {
  resolveCubeCanvasOutputSocketCenterX,
  resolveCubeCanvasOutputSocketRimX,
} from '../../frontend/comfyui/ui/surface/CubeCanvasBoundaryGeometry.js';

describe('CubeCanvasPortRenderer primitives', () => {
  test('elides long input types while retaining short media labels', () => {
    const measure = (value: string): number => value.length * 8;

    expect(elideCubePortLabel('IMAGE', 72, measure)).toBe('IMAGE');
    expect(elideCubePortLabel('VERY_LONG_CONDITIONING_TYPE', 72, measure)).toBe('VERY_LON…');
  });

  test('keeps a fixed label branch while the orthogonal vertical branch follows the socket', () => {
    expect(resolveCubeOutputLeader(420, 100, 500, 180, 460)).toEqual([
      [420, 100],
      [460, 100],
      [460, 180],
      [500, 180],
    ]);
    expect(resolveCubeOutputLeader(420, 100, 500, 240, 460)).toEqual([
      [420, 100],
      [460, 100],
      [460, 240],
      [500, 240],
    ]);
    expect(resolveCubeOutputLeader(420, 100, 500, 60, 460)).toEqual([
      [420, 100],
      [460, 100],
      [460, 100],
      [500, 100],
    ]);
  });

  test('terminates output association leaders at the native socket rim', () => {
    const center = resolveCubeCanvasOutputSocketCenterX(500);

    expect(center).toBe(491);
    expect(resolveCubeCanvasOutputSocketRimX(500)).toBe(487);
  });
});
