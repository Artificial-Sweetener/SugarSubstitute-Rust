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
/** Logical rows and genuinely blank separator destinations are derived without browser geometry or grammar scans. */
import { requireValue } from '../required.js';
import { physicalLines } from '../prompt/physical-lines.js';
import { freezeStructure } from '../prompt/text.js';
import type {
  EmptyReorderPartition,
  PreparedValueReorder,
  ReorderGap,
  ReorderDropTarget,
  ReorderLayout,
  ReorderRow,
  ReorderState,
  ReorderText,
} from './contracts.js';
/** Return exact UTF-16 starts of empty physical lines within an owned separator. */
export function reorderBlankOffsets(text: string): readonly number[] {
  const lines = [...physicalLines(text)];
  return lines
    .slice(1)
    .filter((line) => line.ending && /^[ \t]*[\r\n]/u.test(line.text))
    .map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0));
}
/** Locate immutable parsed region fences through source pieces, retaining only separator lines on the endpoint's side in carrier UTF-16 units. */
function fencedBlankOffsets(
  carrier: ReorderText,
  partition: number,
  side: 'before' | 'after',
): readonly number[] {
  const range =
    carrier.scope.document.regions.separators[side === 'before' ? partition : partition - 1]
      ?.lineRange;
  if (!range) return [];
  let cursor = 0,
    boundary: number | null = null;
  for (const piece of carrier.pieces) {
    if (piece.kind === 'source') {
      if (range.from >= piece.sourceRange.from && range.to <= piece.sourceRange.to) {
        boundary = cursor + (side === 'before' ? range.from : range.to) - piece.sourceRange.from;
        break;
      }
      cursor += piece.sourceRange.to - piece.sourceRange.from;
    } else cursor += piece.text.length;
  }
  if (boundary === null) return [];
  return [...physicalLines(carrier.text)]
    .slice(1)
    .filter((line) => line.ending && /^[ \t,]*(?:\r\n|\r|\n)$/u.test(line.text))
    .map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0))
    .filter((offset) => (side === 'before' ? offset < boundary : offset >= boundary));
}
/** Resolve the held chip's vacancy in the current hidden topology; carrier offsets use separator UTF-16 units. */
export function valueReorderVacancy(
  order: ReorderState,
  hidden: ReorderState,
  held: number,
): ReorderDropTarget | null {
  const remainder = order.order.filter((chip) => chip !== held);
  if (
    hidden.order.length !== remainder.length ||
    hidden.order.some((chip, at) => chip !== remainder[at])
  )
    return null;
  const row = valueReorderLayout(order).rows.find((r) => r.chips.includes(held));
  if (!row) return null;
  const layout = valueReorderLayout(hidden),
    at = row.chips.indexOf(held),
    next = row.chips[at + 1],
    previous = row.chips[at - 1],
    survivor = next ?? previous;
  if (survivor !== undefined) {
    const destination = layout.rows.find((r) => r.chips.includes(survivor));
    return destination
      ? {
          kind: 'row',
          row: destination.index,
          insertion: destination.chips.indexOf(survivor) + (next === undefined ? 1 : 0),
        }
      : null;
  }
  if (!layout.rows.some((r) => r.partition === row.partition))
    return { kind: 'partition', partition: row.partition };
  const index = order.order.indexOf(held),
    before = order.order[index - 1],
    carrier = index === 0 ? 'prefix' : index === order.order.length - 1 ? 'suffix' : 'slot',
    slot = before === undefined ? -1 : hidden.order.indexOf(before),
    offset =
      index === 0
        ? order.prefix.text.length
        : carrier === 'suffix'
          ? hidden.suffix.text.length - order.suffix.text.length
          : requireValue(order.slots[index - 1], 'layout').text.length,
    gap = layout.gaps.find(
      (g) =>
        g.carrier === carrier &&
        (carrier !== 'slot' || g.slot === slot) &&
        g.partition === row.partition,
    ),
    blank = gap?.blankOffsets.indexOf(offset);
  return gap && blank !== undefined && blank >= 0 ? { kind: 'gap', gap: gap.index, blank } : null;
}
/**
 * A directional step leaving a blank/partition vacancy crosses the adjacent chip at the near row boundary; row interiors and far boundaries retain their candidate.
 * @param direction Source-order polarity: +1 crosses forward and -1 crosses backward. The editor supplies the candidate from its chosen adjacent physical lane for vertical steps.
 */
export function valueReorderAdjacentTarget(
  hidden: ReorderState,
  origin: ReorderDropTarget,
  candidate: ReorderDropTarget,
  direction: -1 | 1,
): ReorderDropTarget | null {
  if (origin.kind === 'row' || candidate.kind !== 'row') return candidate;
  const row = valueReorderLayout(hidden).rows[candidate.row];
  if (!row) return null;
  const insertion =
    candidate.insertion +
    (direction > 0 && candidate.insertion === 0
      ? 1
      : direction < 0 && candidate.insertion === row.chips.length
        ? -1
        : 0);
  return { kind: 'row', row: row.index, insertion };
}
/** Derive current rows and separator destinations; edge comma-only lines retain authored punctuation and UTF-16 offsets. */
export function valueReorderLayout(state: ReorderState): ReorderLayout {
  const rows: ReorderRow[] = [],
    gaps: ReorderGap[] = [];
  let start = 0;
  for (let offset = 0; offset < state.order.length; offset++) {
    const id = requireValue(state.order[offset], 'layout');
    const next = state.order[offset + 1];
    const slot = state.slots[offset]?.text ?? '';
    const partition = requireValue(state.partitions[id], 'layout');
    if (next !== undefined && state.partitions[next] === partition && !/[\r\n]/u.test(slot))
      continue;
    rows.push({
      index: rows.length,
      partition,
      start,
      chips: state.order.slice(start, offset + 1),
    });
    if (next !== undefined && state.partitions[next] === partition && /[\r\n]/u.test(slot))
      gaps.push({
        index: gaps.length,
        partition,
        slot: offset,
        carrier: 'slot',
        text: slot,
        blankOffsets: reorderBlankOffsets(slot),
      });
    else if (next !== undefined && state.partitions[next] !== partition) {
      for (const side of ['before', 'after'] as const) {
        const blankOffsets = fencedBlankOffsets(
          requireValue(state.slots[offset], 'layout'),
          side === 'before' ? partition : requireValue(state.partitions[next], 'layout'),
          side,
        );
        if (blankOffsets.length)
          gaps.push({
            index: gaps.length,
            partition:
              side === 'before' ? partition : requireValue(state.partitions[next], 'layout'),
            slot: offset,
            carrier: 'slot',
            text: slot,
            blankOffsets,
          });
      }
    }
    start = offset + 1;
  }
  for (const carrier of ['prefix', 'suffix'] as const) {
    const text = state[carrier].text;
    if (!/[\r\n]/u.test(text) || !state.order.length) continue;
    const partition = requireValue(
      state.partitions[
        carrier === 'prefix'
          ? requireValue(state.order[0], 'layout')
          : requireValue(state.order.at(-1), 'layout')
      ],
      'layout',
    );
    const blankOffsets = /^[ \t,\r\n]*$/u.test(text)
      ? [...physicalLines(text, 'include-eof')]
          .filter((line) => /^[ \t,]*(?:\r\n|\r|\n)?$/u.test(line.text))
          .map((line) => line.from + (line.text.match(/^[ \t]*/u)?.[0].length ?? 0))
      : fencedBlankOffsets(state[carrier], partition, carrier === 'prefix' ? 'after' : 'before');
    if (!blankOffsets.length) continue;
    gaps.push({
      index: gaps.length,
      partition,
      slot: carrier === 'prefix' ? -1 : state.slots.length,
      carrier,
      text,
      blankOffsets,
    });
  }
  return freezeStructure({ rows, gaps });
}
/** Locate original immutable region lines in structural carriers only, never by parsing current text or chip payload. */
export function emptyValueReorderPartitions(
  prepared: PreparedValueReorder,
  state: ReorderState,
): readonly EmptyReorderPartition[] {
  const carriers = [state.prefix, ...state.slots, state.suffix].map((value) => value.text);
  const structural = carriers.join('');
  const locations: {
    readonly from: number;
    readonly to: number;
    readonly ending: string;
  }[] = [];
  let cursor = 0;
  for (const separator of prepared.scope.document.regions.separators) {
    const source = prepared.scope.document.sourceText.slice(
      separator.lineRange.from,
      separator.lineRange.to,
    );
    const from = structural.indexOf(source, cursor);
    if (from < 0) return [];
    const ending = source.match(/(?:\r\n|\r|\n)$/u)?.[0] ?? '';
    locations.push({ from, to: from + source.length, ending });
    cursor = from + source.length;
  }
  const empty: EmptyReorderPartition[] = [];
  for (const partition of prepared.scope.document.regions.partitions) {
    if (state.order.some((chip) => state.partitions[chip] === partition.index)) continue;
    const begin =
      partition.index === 0 ? 0 : requireValue(locations[partition.index - 1], 'layout').to;
    let offset = begin,
      carrier = 0;
    while (
      carrier < carriers.length - 1 &&
      offset > requireValue(carriers[carrier], 'layout').length
    )
      offset -= requireValue(carriers[carrier++], 'layout').length;
    const nextHeader = locations[partition.index];
    const needsEnding = nextHeader?.from === begin;
    empty.push({
      partition: partition.index,
      carrier: carrier === 0 ? 'prefix' : carrier === carriers.length - 1 ? 'suffix' : 'slot',
      slot: carrier - 1,
      offset,
      ending: needsEnding
        ? locations[partition.index - 1]?.ending || nextHeader?.ending || '\n'
        : '',
    });
  }
  return freezeStructure(empty);
}
