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
/** Verify renderer-neutral Cube resizing from every edge and corner. */

import {
  CUBE_RESIZE_EDGES,
  resizeCubeFrame,
} from '../../../frontend/comfyui/ui/cube/geometry/CubeResizeGeometry.js';

describe('resizeCubeFrame', () => {
  test.each([
    ['n', [100, 70], [400, 330]],
    ['ne', [100, 70], [450, 330]],
    ['e', [100, 100], [450, 300]],
    ['se', [100, 100], [450, 270]],
    ['s', [100, 100], [400, 270]],
    ['sw', [150, 100], [350, 270]],
    ['w', [150, 100], [350, 300]],
    ['nw', [150, 70], [350, 330]],
  ] as const)(
    'resizes from %s while anchoring the opposite sides',
    (edge, expectedPosition, expectedSize) => {
      expect(
        resizeCubeFrame({
          edge,
          startPosition: [100, 100],
          startSize: [400, 300],
          delta: [50, -30],
          minimumSize: [320, 180],
        }),
      ).toEqual({
        position: expectedPosition,
        size: expectedSize,
      });
    },
  );

  test('keeps the opposite corner anchored when minimum size clamps a northwest drag', () => {
    expect(
      resizeCubeFrame({
        edge: 'nw',
        startPosition: [100, 100],
        startSize: [400, 300],
        delta: [500, 500],
        minimumSize: [320, 180],
      }),
    ).toEqual({
      position: [180, 220],
      size: [320, 180],
    });
  });

  test('defines every edge exactly once', () => {
    expect(CUBE_RESIZE_EDGES).toEqual(['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw']);
  });
});
