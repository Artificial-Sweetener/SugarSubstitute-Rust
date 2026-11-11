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
/** Actual measured chip/gap rectangles adapt Core topology to pointer and directional destinations. */
import { requireValue } from '../../core/src/required.js';
import {
  valueReorderLayout,
  valueReorderVacancy,
  type ReorderState,
} from '../../core/src/index.js';
/** Viewport CSS pixels are observed only during CodeMirror's read phase. */
export interface ReorderRect {
  readonly kind: 'chip' | 'gap' | 'partition';
  readonly index: number;
  readonly blank?: number;
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}
/** A physical insertion band is independent of text metrics and clipped to the actual editor viewport. */
export interface ReorderMarkerRect {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}
/** Core-issued semantic identity selects exact chip, blank-gap or regional-partition native targets. */
export type ReorderMarkerIdentity = Pick<ReorderRect, 'kind' | 'index' | 'blank'>;
/** Held paint wins; one exact Core vacancy witness supplies the original leading/trailing fallback. */
export interface ReorderMarkerWitnesses {
  readonly held: number;
  readonly fallback: ReorderMarkerIdentity | null;
  readonly trailing: boolean;
}
/** Resolve semantic witnesses before any style, bounds or descendant observation. */
export function reorderMarkerWitnesses(
  order: ReorderState,
  hidden: ReorderState,
  held: number,
): ReorderMarkerWitnesses | null {
  const target = valueReorderVacancy(order, hidden, held);
  if (!target) return null;
  if (target.kind === 'row') {
    const row = requireValue(valueReorderLayout(hidden).rows[target.row], 'reorder-geometry'),
      next = row.chips[target.insertion],
      previous = row.chips[target.insertion - 1];
    const index = next ?? previous;
    return {
      held,
      fallback: index === undefined ? null : { kind: 'chip', index },
      trailing: next === undefined,
    };
  }
  return {
    held,
    fallback:
      target.kind === 'partition'
        ? { kind: 'partition', index: target.partition }
        : { kind: 'gap', index: target.gap, blank: target.blank },
    trailing: false,
  };
}
/** Blank ordinals remain part of identity; equal gap indexes never merge separate native occurrences. */
function sameMarkerIdentity(a: ReorderMarkerIdentity, b: ReorderMarkerIdentity): boolean {
  return a.kind === b.kind && a.index === b.index && (a.kind !== 'gap' || a.blank === b.blank);
}
/** Only these actual targets may be observed for marker paint; all fragments of each selected target remain complete. */
export function selectReorderMarkerTargets<T extends ReorderMarkerIdentity>(
  targets: readonly T[],
  witnesses: ReorderMarkerWitnesses,
): readonly T[] {
  return targets.filter(
    (target) =>
      (target.kind === 'chip' && target.index === witnesses.held) ||
      (!!witnesses.fallback && sameMarkerIdentity(target, witnesses.fallback)),
  );
}
/** Resolve the unclipped current vacancy from Core topology and actual measured fragments. */
export function reorderHeldSlot(
  order: ReorderState,
  hidden: ReorderState,
  held: number,
  rects: readonly ReorderRect[],
): ReorderMarkerRect | null {
  const witnesses = reorderMarkerWitnesses(order, hidden, held);
  if (!witnesses) return null;
  const footprint = rects.find((rect) => rect.kind === 'chip' && rect.index === held);
  if (footprint) {
    if (
      ![footprint.left, footprint.right, footprint.top, footprint.bottom].every(Number.isFinite) ||
      footprint.right <= footprint.left ||
      footprint.bottom <= footprint.top
    )
      return null;
    return {
      left: footprint.left,
      right: footprint.left + 2,
      top: footprint.top,
      bottom: footprint.bottom,
    };
  }
  const candidates = witnesses.fallback
    ? rects.filter((rect) =>
        sameMarkerIdentity(rect, requireValue(witnesses.fallback, 'reorder-geometry')),
      )
    : [];
  const hit = witnesses.trailing ? candidates.at(-1) : candidates[0];
  const x = witnesses.trailing ? hit?.right : hit?.left;
  if (
    !hit ||
    x === undefined ||
    ![x, hit.left, hit.right, hit.top, hit.bottom].every(Number.isFinite)
  )
    return null;
  if (hit.right <= hit.left || hit.bottom <= hit.top) return null;
  return { left: x, right: x + 2, top: hit.top, bottom: hit.bottom };
}
/** Clip the same authoritative insertion occurrence only for viewport marker painting. */
export function reorderHeldMarker(
  order: ReorderState,
  hidden: ReorderState,
  held: number,
  rects: readonly ReorderRect[],
  viewport: ReorderMarkerRect,
): ReorderMarkerRect | null {
  const slot = reorderHeldSlot(order, hidden, held, rects);
  if (
    !slot ||
    ![viewport.left, viewport.right, viewport.top, viewport.bottom].every(Number.isFinite)
  )
    return null;
  const result = {
    left: Math.max(slot.left, viewport.left),
    right: Math.min(slot.right, viewport.right),
    top: Math.max(slot.top, viewport.top),
    bottom: Math.min(slot.bottom, viewport.bottom),
  };
  return result.right > result.left && result.bottom > result.top ? result : null;
}
/** Complete actual target fragments and containing viewport, observed in one supported native read. */
export interface ReorderMeasuredGeometry {
  readonly rects: readonly ReorderRect[];
  readonly viewport: ReorderMarkerRect;
}
