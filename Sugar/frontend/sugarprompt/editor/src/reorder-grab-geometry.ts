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
/** The actual held rich fragment footprint uses viewport CSS pixels, including every wrapped/disjoint fragment. */
import type { ReorderRect } from './reorder-geometry.js';
export interface ReorderGrabGeometry {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
  readonly grabX: number;
  readonly grabY: number;
  readonly pressedFragment: number;
  readonly fragments: readonly ReorderRect[];
}
/** Capture an actual press within the measured held fragment union; never invent a center or clamp an authored hotspot. */
export function reorderGrabGeometry(
  rects: readonly ReorderRect[],
  held: number,
  press: { readonly x: number; readonly y: number },
): ReorderGrabGeometry | null {
  const fragments = rects
    .filter(
      (rect) =>
        rect.kind === 'chip' &&
        rect.index === held &&
        [rect.left, rect.right, rect.top, rect.bottom].every(Number.isFinite) &&
        rect.right > rect.left &&
        rect.bottom > rect.top,
    )
    .map((rect) => Object.freeze({ ...rect }));
  const pressedFragment = fragments.findIndex(
    (rect) =>
      press.x >= rect.left &&
      press.x <= rect.right &&
      press.y >= rect.top &&
      press.y <= rect.bottom,
  );
  if (pressedFragment < 0) return null;
  const left = Math.min(...fragments.map((rect) => rect.left)),
    top = Math.min(...fragments.map((rect) => rect.top)),
    right = Math.max(...fragments.map((rect) => rect.right)),
    bottom = Math.max(...fragments.map((rect) => rect.bottom));
  return Object.freeze({
    left,
    top,
    width: right - left,
    height: bottom - top,
    grabX: press.x - left,
    grabY: press.y - top,
    pressedFragment,
    fragments: Object.freeze(fragments),
  });
}
