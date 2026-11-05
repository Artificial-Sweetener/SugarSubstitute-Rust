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
/** Arrange the editing top bar and tool rail around host-owned canvas chrome. */
import {
  CANVAS_EDIT_CHROME_INSET,
  type CanvasEditPoint,
  type CanvasEditRect,
} from './CanvasToolbarPlacement.js';

/** Space kept between neighbouring chrome surfaces. */
export const CANVAS_EDIT_CHROME_GAP = 8;

export interface CanvasEditSize {
  readonly width: number;
  readonly height: number;
}

/** Top-left points of the dock's surfaces, in the same space as the obstacles. */
export interface CanvasEditDockPlacement {
  readonly topBar: CanvasEditPoint;
  readonly rail: CanvasEditPoint;
  /** Height the rail may take before the first host surface below it, or null when none is. */
  readonly railHeight: number | null;
}

/**
 * Place the top bar beside, and the rail below, the host surfaces they would cover.
 *
 * Both start at the chrome inset. The top bar (history, then the active tool's options) moves
 * right past each intersecting host surface, so a host surface in the corner and the options
 * share the top row. The rail then moves down past each intersecting host surface that reaches
 * its top, including the top bar. A host surface further down the rail's column, such as the
 * canvas navigation bar, instead bounds the rail's height so the two never overlap. Empty sizes
 * and obstacles never intersect.
 */
export function canvasEditDockPlacement(
  obstacles: readonly CanvasEditRect[],
  topBar: CanvasEditSize,
  rail: CanvasEditSize,
): CanvasEditDockPlacement {
  const bar = { left: CANVAS_EDIT_CHROME_INSET, top: CANVAS_EDIT_CHROME_INSET, ...topBar };
  for (const obstacle of [...obstacles].sort((a, b) => a.left - b.left))
    if (intersects(bar, obstacle))
      bar.left = obstacle.left + obstacle.width + CANVAS_EDIT_CHROME_GAP;
  const column = { left: CANVAS_EDIT_CHROME_INSET, top: CANVAS_EDIT_CHROME_INSET, ...rail };
  for (const obstacle of [...obstacles, bar].sort((a, b) => a.top - b.top))
    if (obstacle.top <= column.top && intersects(column, obstacle))
      column.top = obstacle.top + obstacle.height + CANVAS_EDIT_CHROME_GAP;
  const limits = obstacles
    .filter(
      (obstacle) =>
        obstacle.width > 0 &&
        obstacle.height > 0 &&
        obstacle.top > column.top &&
        obstacle.left < column.left + column.width &&
        column.left < obstacle.left + obstacle.width,
    )
    .map((obstacle) => obstacle.top - CANVAS_EDIT_CHROME_GAP - column.top);
  return {
    topBar: { x: bar.left, y: bar.top },
    rail: { x: column.left, y: column.top },
    railHeight: limits.length ? Math.max(0, Math.min(...limits)) : null,
  };
}

/** Report whether two rectangles share interior area; an empty rectangle shares none. */
function intersects(a: CanvasEditRect, b: CanvasEditRect): boolean {
  if (a.width <= 0 || a.height <= 0 || b.width <= 0 || b.height <= 0) return false;
  return (
    a.left < b.left + b.width &&
    b.left < a.left + a.width &&
    a.top < b.top + b.height &&
    b.top < a.top + a.height
  );
}
