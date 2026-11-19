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
/**
 * Decide where a dragged item lands among its neighbours, from settled geometry alone.
 *
 * Every reorder gesture in SugarCubes resolves its slot here. Nothing in this module reads the
 * page: a slot is computed from extents and positions the caller already holds, so a neighbour that
 * is still animating towards its place never moves the answer.
 */

/** A transient permutation of items laid end to end, without changing their accepted order. */
export interface ExtentPlacement {
  /** The index the dragged item would take. */
  readonly target: number;
  /** How far each item is displaced from its accepted position, by accepted index. */
  readonly offsets: readonly number[];
  /** The dragged item's leading edge, kept inside the occupied extent. */
  readonly draggedStart: number;
}

/** Where each item starts when laid end to end, with the terminal edge as the last entry. */
export function slotOffsets(extents: readonly number[]): readonly number[] {
  const offsets = [0];
  for (const extent of extents) offsets.push((offsets.at(-1) ?? 0) + extent);
  return offsets;
}

/**
 * Place the item at `origin` after it has travelled `delta` along the items' axis.
 *
 * Items may have unequal extents. The target moves one slot at a time from `previousTarget`, and
 * only once the dragged edge passes a slot's midpoint by `deadBand`, so a pointer resting on a
 * boundary does not flip the order back and forth.
 */
export function extentPlacement(
  extents: readonly number[],
  origin: number,
  delta: number,
  previousTarget = origin,
  deadBand = 6,
): ExtentPlacement {
  if (
    !extents.length ||
    origin < 0 ||
    origin >= extents.length ||
    extents.some((extent) => !Number.isFinite(extent) || extent <= 0)
  )
    throw new Error('Reorder geometry is unavailable.');
  const starts = slotOffsets(extents);
  const order = extents.map((_extent, index) => index).filter((index) => index !== origin);
  const slots = slotOffsets(order.map((index) => extents[index] ?? 0));
  const draggedStart = Math.max(0, Math.min(slots.at(-1) ?? 0, (starts[origin] ?? 0) + delta));
  let target = Math.max(0, Math.min(extents.length - 1, previousTarget));
  while (
    target < extents.length - 1 &&
    draggedStart >= ((slots[target] ?? 0) + (slots[target + 1] ?? 0)) / 2 + deadBand
  )
    target++;
  while (
    target > 0 &&
    draggedStart <= ((slots[target - 1] ?? 0) + (slots[target] ?? 0)) / 2 - deadBand
  )
    target--;
  order.splice(target, 0, origin);
  const destinations = slotOffsets(order.map((index) => extents[index] ?? 0));
  return {
    target,
    draggedStart,
    offsets: extents.map((_extent, index) =>
      index === origin
        ? draggedStart - (starts[origin] ?? 0)
        : (destinations[order.indexOf(index)] ?? 0) - (starts[index] ?? 0),
    ),
  };
}

/** A list of equal rows a fixed stride apart, and the lifted row's centre along its axis. */
export interface StrideProjectionInput {
  /** The order shown now: the accepted order, or the previous projection of this drag. */
  readonly current: readonly string[];
  /** Whether an item may change place. An item that may not keeps its slot. */
  movable(key: string): boolean;
  /** The lifted item. */
  readonly key: string;
  /** The lifted row's centre, in the same coordinates as `listStart`. */
  readonly center: number;
  /** Where the first row starts. */
  readonly listStart: number;
  /** The distance from one row's start to the next. */
  readonly stride: number;
  /** One row's extent along the axis. */
  readonly size: number;
}

/**
 * Project the lifted row into a list of equal rows, or return null when the order does not change.
 *
 * Slots are measured against the order shown now rather than the accepted one. The lifted row
 * therefore has to cross a whole stride to move back, which keeps the list still while the pointer
 * trembles on a boundary.
 */
export function strideProjection(input: StrideProjectionInput): readonly string[] | null {
  const { current, key } = input;
  const movable = new Set(current.filter((item) => input.movable(item)));
  const remaining = current.filter((item) => movable.has(item) && item !== key);
  let index = remaining.length;
  for (const [slot, item] of remaining.entries()) {
    const destination = current.indexOf(item) * input.stride + input.size / 2;
    if (input.center < input.listStart + destination) {
      index = slot;
      break;
    }
  }
  remaining.splice(index, 0, key);
  let cursor = 0;
  const projected = current.map((item) =>
    movable.has(item) ? (remaining[cursor++] ?? item) : item,
  );
  return projected.every((item, slot) => item === current[slot]) ? null : projected;
}
