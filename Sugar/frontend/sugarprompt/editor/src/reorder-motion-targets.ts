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
/** Immutable actual viewport-CSS-pixel geometry prepares translated targets independently of DOM and lifetime ownership. */
import type { ReorderRect } from './reorder-geometry.js';
import type { ReorderMotionPoint, ReorderMotionTarget } from './reorder-motion-clock.js';
/** Group complete observed fragments into the existing translation anchor, preserving first-observed identity order. */
function anchors(rects: readonly ReorderRect[]): ReadonlyMap<number, ReorderMotionPoint> {
  const result = new Map<number, ReorderMotionPoint>();
  for (const rect of rects) {
    if (rect.kind !== 'chip') continue;
    if (
      !Number.isSafeInteger(rect.index) ||
      rect.index < 0 ||
      ![rect.left, rect.right, rect.top, rect.bottom].every(Number.isFinite)
    )
      throw new TypeError('reorder-motion-geometry');
    const previous = result.get(rect.index);
    result.set(
      rect.index,
      Object.freeze({
        left: Math.min(previous?.left ?? Infinity, rect.left),
        top: Math.min(previous?.top ?? Infinity, rect.top),
      }),
    );
  }
  return result;
}
/** Select at most 64 actually translated groups; missing origins and identical anchors remain native, while excess changed output refuses the entire animation. */
export function reorderMotionTargets(
  origin: readonly ReorderRect[],
  geometry: readonly ReorderRect[],
  keyboardHeld: number | null,
): readonly ReorderMotionTarget[] | null {
  const previous = anchors(origin),
    targets: ReorderMotionTarget[] = [];
  for (const [id, to] of anchors(geometry)) {
    const from = previous.get(id);
    if (!from || (from.left === to.left && from.top === to.top)) continue;
    targets.push(
      Object.freeze({
        id,
        kind: id === keyboardHeld ? 'held' : 'peer',
        from,
        to,
      }),
    );
    if (targets.length > 64) return null;
  }
  return Object.freeze(targets);
}
