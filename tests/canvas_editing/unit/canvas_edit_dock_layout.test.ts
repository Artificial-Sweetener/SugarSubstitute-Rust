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
/** The editing dock shares the canvas corner with the route tabs without covering them. */
import { canvasEditDockPlacement } from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasEditDockLayout.js';

interface Box {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

const ROUTE_TABS: Box = { left: 8, top: 8, width: 140, height: 40 };
const HISTORY = { width: 64, height: 36 };
const OPEN_BRUSH_OPTIONS = { width: 260, height: 150 };
const RAIL = { width: 40, height: 260 };

function overlaps(a: Box, b: Box): boolean {
  return (
    a.left < b.left + b.width &&
    b.left < a.left + a.width &&
    a.top < b.top + b.height &&
    b.top < a.top + a.height
  );
}

function boxes(obstacles: readonly Box[], topBar: typeof HISTORY): readonly Box[] {
  const placed = canvasEditDockPlacement(obstacles, topBar, RAIL);
  return [
    ...obstacles,
    { left: placed.topBar.x, top: placed.topBar.y, ...topBar },
    { left: placed.rail.x, top: placed.rail.y, ...RAIL },
  ];
}

test('without host chrome the top bar holds the corner and the rail sits beneath it', () => {
  expect(canvasEditDockPlacement([], HISTORY, RAIL)).toEqual({
    topBar: { x: 8, y: 8 },
    rail: { x: 8, y: 52 },
  });
  expect(canvasEditDockPlacement([], OPEN_BRUSH_OPTIONS, RAIL).rail).toEqual({ x: 8, y: 166 });
});

test('the top bar follows the route tabs along the top row and the rail drops below them', () => {
  expect(canvasEditDockPlacement([ROUTE_TABS], HISTORY, RAIL)).toEqual({
    topBar: { x: 156, y: 8 },
    rail: { x: 8, y: 56 },
  });
});

test('open tool options stay beside the route tabs and leave the rail where it was', () => {
  expect(canvasEditDockPlacement([ROUTE_TABS], OPEN_BRUSH_OPTIONS, RAIL)).toEqual({
    topBar: { x: 156, y: 8 },
    rail: { x: 8, y: 56 },
  });
});

test('no two surfaces overlap for collapsed or open options, with or without route tabs', () => {
  for (const obstacles of [[], [ROUTE_TABS], [{ ...ROUTE_TABS, width: 24, height: 60 }]])
    for (const topBar of [HISTORY, OPEN_BRUSH_OPTIONS]) {
      const placed = boxes(obstacles, topBar);
      for (const [index, box] of placed.entries())
        for (const other of placed.slice(index + 1)) expect(overlaps(box, other)).toBe(false);
    }
});

test('unmeasured surfaces and empty obstacles are never treated as covering anything', () => {
  expect(
    canvasEditDockPlacement(
      [{ left: 8, top: 8, width: 0, height: 40 }],
      { width: 0, height: 0 },
      RAIL,
    ),
  ).toEqual({ topBar: { x: 8, y: 8 }, rail: { x: 8, y: 8 } });
});
