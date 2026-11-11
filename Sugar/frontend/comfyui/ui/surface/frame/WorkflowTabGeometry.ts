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
/** Calculate committed workflow-tab positions independently of DOM animation. */

/** Describe a transient permutation without changing the host's workflow order. */
export interface WorkflowTabDragPlacement {
  readonly target: number;
  readonly offsets: readonly number[];
  readonly draggedLeft: number;
}

/** Include the terminal edge so every consumer shares the same starts and total extent. */
export function workflowTabOffsets(widths: readonly number[]): readonly number[] {
  const offsets = [0];
  for (const width of widths) offsets.push((offsets.at(-1) ?? 0) + width);
  return offsets;
}

/** Resolve a scroll position against actual tab extents, including unequal name-sized tabs. */
export function workflowTabAtOffset(widths: readonly number[], position: number): number {
  const offsets = workflowTabOffsets(widths);
  const index = widths.findIndex((_width, index) => (offsets[index + 1] ?? 0) > position + 0.5);
  return index < 0 ? Math.max(0, widths.length - 1) : index;
}

/** Reorder from immutable insertion slots with a small dead band, independent of painted rectangles. */
export function workflowTabDragPlacement(
  widths: readonly number[],
  origin: number,
  delta: number,
  previousTarget = origin,
): WorkflowTabDragPlacement {
  if (
    !widths.length ||
    origin < 0 ||
    origin >= widths.length ||
    widths.some((width) => !Number.isFinite(width) || width <= 0)
  )
    throw new Error('Workflow tab geometry is unavailable.');
  const starts = workflowTabOffsets(widths);
  const order = widths.map((_width, index) => index).filter((index) => index !== origin);
  const slots = workflowTabOffsets(order.map((index) => widths[index] ?? 0));
  const draggedLeft = Math.max(0, Math.min(slots.at(-1) ?? 0, (starts[origin] ?? 0) + delta));
  let target = Math.max(0, Math.min(widths.length - 1, previousTarget));
  while (
    target < widths.length - 1 &&
    draggedLeft >= ((slots[target] ?? 0) + (slots[target + 1] ?? 0)) / 2 + 6
  )
    target++;
  while (target > 0 && draggedLeft <= ((slots[target - 1] ?? 0) + (slots[target] ?? 0)) / 2 - 6)
    target--;
  order.splice(target, 0, origin);
  const destinations = workflowTabOffsets(order.map((index) => widths[index] ?? 0));
  return {
    target,
    draggedLeft,
    offsets: widths.map((_width, index) =>
      index === origin
        ? draggedLeft - (starts[origin] ?? 0)
        : (destinations[order.indexOf(index)] ?? 0) - (starts[index] ?? 0),
    ),
  };
}
