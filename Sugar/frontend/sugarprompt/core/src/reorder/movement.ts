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
/** Pure held-chip removal and typed row/gap insertion preserve source-owned structural carriers. */
import { requireValue } from '../required.js';
import { freezeStructure } from '../prompt/text.js';
import type {
  PreparedValueReorder,
  ReorderDropTarget,
  ReorderRow,
  ReorderText,
  ReorderState,
} from './contracts.js';
import { emptyValueReorderPartitions, valueReorderLayout } from './layout.js';
import { concatText, derivedText, sliceText } from './source-pieces.js';
import { validReorderState } from './serialization.js';
/** Merge source-owned separators; an adjacent-LoRA horizontal carrier supplies padding without another comma. */
function mergeSlots(left: ReorderText, right: ReorderText): ReorderText | null {
  if (!right.text.startsWith(',')) {
    if (/[\r\n]/u.test(right.text))
      return /[\r\n]/u.test(left.text) ? concatText(left, right) : right;
    if (!/[,\r\n]/u.test(left.text)) return right.text ? right : left;
    if (!/^[ \t]*$/u.test(right.text)) return null;
  }
  let suffix = right.text.startsWith(',') ? sliceText(right, 1) : right;
  if (/[\r\n]/u.test(left.text))
    suffix = sliceText(suffix, suffix.text.match(/^[ \t]+/u)?.[0].length ?? 0);
  if (
    !/[\r\n]/u.test(left.text) &&
    suffix.text &&
    /^[ \t]+$/u.test(suffix.text) &&
    /[ \t]$/u.test(left.text)
  )
    return left;
  return concatText(left, suffix);
}
/** Remove one actual item for preview target preparation; this does not publish editable source. */
export function hideReorderChip(
  prepared: PreparedValueReorder,
  state: ReorderState,
  chip: number,
): ReorderState | null {
  if (!validReorderState(prepared, state)) return null;
  const at = state.order.indexOf(chip);
  if (at < 0) return null;
  const order = [...state.order],
    slots = [...state.slots];
  let prefix = state.prefix,
    suffix = state.suffix;
  order.splice(at, 1);
  if (slots.length) {
    if (at === 0) {
      if (
        state.partitions[requireValue(state.order[1], 'movement')] !== state.partitions[chip] ||
        /[\r\n]/u.test(requireValue(slots[0], 'movement').text)
      )
        prefix = concatText(
          prefix,
          sliceText(
            requireValue(slots[0], 'movement'),
            requireValue(slots[0], 'movement').text.match(/^,[ \t]*/u)?.[0].length ?? 0,
          ),
        );
      slots.shift();
    } else if (at === state.order.length - 1) {
      if (
        state.partitions[requireValue(state.order[at - 1], 'movement')] !==
          state.partitions[chip] ||
        /[\r\n]/u.test(requireValue(slots.at(-1), 'movement').text)
      )
        suffix = concatText(
          sliceText(
            requireValue(slots.at(-1), 'movement'),
            requireValue(slots.at(-1), 'movement').text.match(/^,[ \t]*/u)?.[0].length ?? 0,
          ),
          suffix,
        );
      slots.pop();
    } else {
      const merged = mergeSlots(
        requireValue(slots[at - 1], 'movement'),
        requireValue(slots[at], 'movement'),
      );
      if (merged === null) return null;
      slots[at - 1] = merged;
      slots.splice(at, 1);
    }
  }
  return freezeStructure({ ...state, order, slots, prefix, suffix });
}
/** Preserve the destination row's established comma, spacing or adjacent-LoRA style. */
function rowSeparator(state: ReorderState, row: ReorderRow): ReorderText {
  const slots = state.slots.slice(row.start, row.start + row.chips.length - 1);
  const comma = slots.find((slot) => slot.text.includes(',') && !/[\r\n]/u.test(slot.text));
  if (comma !== undefined) return comma;
  if (slots.some((slot) => !!slot.text))
    return derivedText(state.prefix.scope, ' ', 'row-separator');
  return derivedText(state.prefix.scope, slots.length ? '' : ', ', 'row-separator');
}
/** Insert the held original chip into an actual prepared row or internal blank-line target. */
export function dropReorderChip(
  prepared: PreparedValueReorder,
  hidden: ReorderState,
  chip: number,
  target: ReorderDropTarget,
): ReorderState | null {
  if (
    !validReorderState(prepared, hidden, false) ||
    hidden.order.length !== prepared.chips.length - 1 ||
    hidden.order.includes(chip) ||
    !prepared.chips[chip]
  )
    return null;
  const { rows, gaps } = valueReorderLayout(hidden);
  const order = [...hidden.order],
    slots = [...hidden.slots],
    partitions = [...hidden.partitions];
  let prefix = hidden.prefix,
    suffix = hidden.suffix;
  if (target.kind === 'row') {
    const row = rows[target.row];
    if (
      !Number.isSafeInteger(target.row) ||
      !row ||
      !Number.isSafeInteger(target.insertion) ||
      target.insertion < 0 ||
      target.insertion > row.chips.length
    )
      return null;
    const at = row.start + target.insertion;
    order.splice(at, 0, chip);
    if (target.insertion === 0) slots.splice(at, 0, rowSeparator(hidden, row));
    else if (target.insertion === row.chips.length)
      slots.splice(at - 1, 0, rowSeparator(hidden, row));
    else {
      const original = requireValue(slots[at - 1], 'movement');
      slots[at - 1] = rowSeparator(hidden, row);
      slots.splice(at, 0, original);
    }
    partitions[chip] = row.partition;
  } else if (target.kind === 'gap') {
    const gap = gaps[target.gap];
    const offset = gap?.blankOffsets[target.blank];
    if (
      !Number.isSafeInteger(target.gap) ||
      !Number.isSafeInteger(target.blank) ||
      !gap ||
      offset === undefined
    )
      return null;
    const carrier =
      gap.carrier === 'prefix'
        ? hidden.prefix
        : gap.carrier === 'suffix'
          ? hidden.suffix
          : requireValue(hidden.slots[gap.slot], 'movement');
    if (gap.carrier === 'prefix') {
      prefix = sliceText(carrier, 0, offset);
      slots.unshift(sliceText(carrier, offset));
      order.unshift(chip);
    } else if (gap.carrier === 'suffix') {
      slots.push(sliceText(carrier, 0, offset));
      suffix = sliceText(carrier, offset);
      order.push(chip);
    } else {
      order.splice(gap.slot + 1, 0, chip);
      slots[gap.slot] = sliceText(carrier, 0, offset);
      slots.splice(
        gap.slot + 1,
        0,
        concatText(
          derivedText(prepared.scope, ',', 'blank-line-comma'),
          sliceText(carrier, offset),
        ),
      );
    }
    partitions[chip] = gap.partition;
  } else {
    if (!Number.isSafeInteger(target.partition)) return null;
    const destination = emptyValueReorderPartitions(prepared, hidden).find(
      (item) => item.partition === target.partition,
    );
    if (!destination) return null;
    const carrier =
      destination.carrier === 'prefix'
        ? prefix
        : destination.carrier === 'suffix'
          ? suffix
          : requireValue(slots[destination.slot], 'movement');
    const left = sliceText(carrier, 0, destination.offset),
      right = concatText(
        derivedText(prepared.scope, destination.ending, 'empty-partition-ending'),
        sliceText(carrier, destination.offset),
      );
    if (!order.length) {
      prefix = destination.carrier === 'prefix' ? left : concatText(prefix, left);
      suffix = destination.carrier === 'prefix' ? concatText(right, suffix) : right;
      order.push(chip);
    } else if (destination.carrier === 'prefix') {
      prefix = left;
      slots.unshift(right);
      order.unshift(chip);
    } else if (destination.carrier === 'suffix') {
      suffix = right;
      slots.push(left);
      order.push(chip);
    } else {
      slots[destination.slot] = left;
      slots.splice(destination.slot + 1, 0, right);
      order.splice(destination.slot + 1, 0, chip);
    }
    partitions[chip] = destination.partition;
  }
  const result = freezeStructure({
    ...hidden,
    order,
    slots,
    partitions,
    prefix,
    suffix,
  });
  return validReorderState(prepared, result) ? result : null;
}
/** Prepare a complete immutable move through the same hidden-target contract used by pointer previews. */
export function moveReorderChip(
  prepared: PreparedValueReorder,
  state: ReorderState,
  chip: number,
  target: ReorderDropTarget,
): ReorderState | null {
  const hidden = hideReorderChip(prepared, state, chip);
  if (!hidden) return null;
  const result = dropReorderChip(prepared, hidden, chip, target);
  if (
    result &&
    target.kind === 'row' &&
    result.order.every((id, index) => id === state.order[index])
  ) {
    const originalRow = valueReorderLayout(state).rows.find((row) => row.chips.includes(chip));
    const destination = valueReorderLayout(hidden).rows[target.row];
    if (
      originalRow &&
      destination &&
      originalRow.partition === destination.partition &&
      originalRow.chips.filter((id) => id !== chip).every((id) => destination.chips.includes(id)) &&
      originalRow.chips.length - 1 === destination.chips.length
    )
      return state;
  }
  return result;
}
