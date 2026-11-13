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
/** One READ composes current canonical viewport fragments with the exact sampled passive motion, without collision or source authority. */
import type { EditorView } from '@codemirror/view';
import type { ReorderRect } from './reorder-geometry.js';
import type { ReorderMotionFrame } from './reorder-motion-clock.js';
import type { ReorderMotionScene } from './reorder-motion-scene.js';
import { nativeReorderViewportFootprint } from './reorder-native-paint.js';
/** Immutable pre-transform visual endpoints are display geometry only, never collision targets. */
export type ReorderMotionOrigin = readonly ReorderRect[];
/** A borrowed native publication may retain current unavailability without inventing rectangle output. */
export interface ReorderMotionGeometryLease {
  readonly current: () => boolean;
  readonly rects: readonly ReorderRect[] | null;
}
/** Preserve complete eligible canonical geometry while replacing only actual selected sampled peer fragments. */
export function readReorderMotionOrigin(
  view: EditorView,
  accepted: ReorderMotionGeometryLease | null,
  scene: ReorderMotionScene | null,
  frame: ReorderMotionFrame | null,
): ReorderMotionOrigin {
  const geometry =
      (accepted?.current() && (!scene || accepted.rects === scene.geometry)
        ? accepted.rects
        : null) ??
      nativeReorderViewportFootprint(view) ??
      [],
    points = new Map(frame?.positions.map((point) => [point.id, point])),
    overlays = new Map(
      scene?.chips.map((chip) => {
        const point = points.get(chip.id),
          dx = point ? point.left - chip.left : 0,
          dy = point ? point.top - chip.top : 0;
        return [
          chip.id,
          chip.rects.map((rect) =>
            Object.freeze({
              ...rect,
              left: rect.left + dx,
              right: rect.right + dx,
              top: rect.top + dy,
              bottom: rect.bottom + dy,
            }),
          ),
        ] as const;
      }),
    ),
    result: ReorderRect[] = [];
  const emitted = new Set<number>();
  for (const rect of geometry) {
    const overlay = rect.kind === 'chip' ? overlays.get(rect.index) : undefined;
    if (!overlay) result.push(Object.freeze({ ...rect }));
    else if (!emitted.has(rect.index)) {
      result.push(...overlay);
      emitted.add(rect.index);
    }
  }
  for (const [id, overlay] of overlays) if (!emitted.has(id)) result.push(...overlay);
  return result.length <= 4096 ? Object.freeze(result) : Object.freeze([]);
}
