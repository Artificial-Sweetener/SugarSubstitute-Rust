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
/** Prepare aspect-based LoRA wall rows and bounded visual navigation outside native input and measurement. */
import { requireValue } from '../../core/src/required.js';
import { roundToEven } from './round-to-even.js';
/** One prepared CSS-local tile rectangle retains its ordered candidate index and visual row. */
export interface LoraWallBox {
  readonly index: number;
  readonly row: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}
/** A contiguous visual row provides binary viewport lookup and bounded nearest-centre navigation. */
export interface LoraWallRow {
  readonly from: number;
  readonly to: number;
  readonly top: number;
  readonly height: number;
}
/** Immutable geometry belongs to one candidate ordering and measured CSS-local viewport width. */
export interface LoraWallLayout {
  readonly width: number;
  readonly height: number;
  readonly boxes: readonly LoraWallBox[];
  readonly rows: readonly LoraWallRow[];
}
/** One legal contiguous row records its source-independent visual cost. */
interface RowCandidate {
  readonly to: number;
  readonly height: number;
  readonly widths: readonly number[];
  readonly cost: number;
}
/** The adopted picker uses portrait fallback and bounds extreme thumbnail aspect ratios. */
function aspect(value: number): number {
  return Number.isFinite(value) && value > 0 ? Math.max(0.2, Math.min(6, value)) : 0.72;
}
/** Enumerate a justified row or the deliberate ragged final row under the adopted 198/156/258/126/4 profile. */
function candidate(
  aspects: readonly number[],
  from: number,
  to: number,
  width: number,
  relaxed: boolean,
  ragged: boolean,
): RowCandidate | null {
  const final = to === aspects.length,
    count = to - from;
  if (ragged && !final) return null;
  let sum = 0;
  for (let index = from; index < to; index++)
    sum += requireValue(aspects[index], 'lora-wall-layout');
  const available = width - (count - 1) * 4,
    natural = available / Math.max(sum, 0.001);
  const needsBounds = natural < 156 || natural > 258;
  if (!ragged && needsBounds && (final || !relaxed)) return null;
  const height = ragged
    ? Math.max(1, Math.min(258, Math.min(198, natural)))
    : needsBounds
      ? Math.max(156, Math.min(258, natural))
      : natural;
  const widths = aspects
    .slice(from, to)
    .map((ratio) => (ragged ? Math.max(1, ratio * height) : ratio * height));
  if (!ragged)
    widths[count - 1] = Math.max(
      1,
      requireValue(widths[count - 1], 'lora-wall-layout') +
        available -
        widths.reduce((total, value) => total + value, 0),
    );
  const coverage =
    (widths.reduce((total, value) => total + value, 0) + (count - 1) * 4) / Math.max(width, 1);
  if (!final && Math.abs(coverage - 1) > 0.01) return null;
  let cost = Math.abs(height - 198);
  if (final)
    cost +=
      Math.max(0, (1 - coverage) * width) * 0.18 +
      Math.max(0, Math.max(156 * 0.9, 198 * 0.82) - height) * 6.5 +
      Math.max(0, count - 7) * 65 +
      (ragged ? 18 : 0);
  else cost += Math.abs(height - natural) * 12 + (count === 1 ? 24 : 0);
  return { to, height, widths, cost };
}
/** Select legal contiguous rows with stable reference tie order and final-row height comfort. */
function partition(
  aspects: readonly number[],
  width: number,
  relaxed: boolean,
): readonly RowCandidate[] | null {
  const limit = Math.max(1, Math.floor((width + 4) / 130));
  const costs = Array<number>(aspects.length + 1).fill(Infinity);
  const choices = Array<RowCandidate | null>(aspects.length + 1).fill(null);
  costs[aspects.length] = 0;
  for (let from = aspects.length - 1; from >= 0; from--) {
    for (let to = from + 1; to <= Math.min(aspects.length, from + limit); to++) {
      for (const ragged of [false, true]) {
        const row = candidate(aspects, from, to, width, relaxed, ragged);
        if (!row || costs[to] === Infinity) continue;
        const next = choices[to];
        const transition = next
          ? Math.abs(row.height - next.height) * (next.to === aspects.length ? 1.1 : 0.18)
          : 0;
        const cost = row.cost + transition + requireValue(costs[to], 'lora-wall-layout');
        if (cost < requireValue(costs[from], 'lora-wall-layout') - 0.001) {
          costs[from] = cost;
          choices[from] = row;
        }
      }
    }
  }
  if (costs[0] === Infinity) return null;
  const rows: RowCandidate[] = [];
  for (let from = 0; from < aspects.length; ) {
    const row = choices[from];
    if (!row) throw new Error('LoRA wall partition invariant violated');
    rows.push(row);
    from = row.to;
  }
  return rows;
}
/** Compile the complete ordered wall once per measured width; visible projection never traverses the catalog. */
export function prepareLoraWallLayout(ratios: readonly number[], width: number): LoraWallLayout {
  if (!Number.isFinite(width) || width < 0 || width > 16384 || ratios.length > 100000)
    throw new RangeError('Invalid LoRA wall bounds');
  if (!ratios.length || width === 0)
    return Object.freeze({
      width,
      height: 0,
      boxes: Object.freeze([]),
      rows: Object.freeze([]),
    });
  const aspects = ratios.map(aspect),
    safeWidth = Math.max(1, width);
  const chosen = partition(aspects, safeWidth, false) ?? partition(aspects, safeWidth, true);
  if (!chosen) throw new Error('LoRA wall layout unavailable');
  const boxes: LoraWallBox[] = [],
    rows: LoraWallRow[] = [];
  let from = 0,
    y = 0;
  for (const [rowIndex, row] of chosen.entries()) {
    const height = Math.max(1, roundToEven(row.height));
    rows.push(Object.freeze({ from, to: row.to, top: y, height }));
    let x = 0;
    for (let index = from; index < row.to; index++) {
      const tileWidth = Math.max(
        1,
        roundToEven(requireValue(row.widths[index - from], 'lora-wall-layout')),
      );
      boxes.push(Object.freeze({ index, row: rowIndex, x, y, width: tileWidth, height }));
      x += tileWidth + 4;
    }
    from = row.to;
    y += height + 4;
  }
  return Object.freeze({
    width,
    height: Math.max(0, y - 4),
    boxes: Object.freeze(boxes),
    rows: Object.freeze(rows),
  });
}
/** Move vertically to the nearest tile centre in the adjacent visual row, wrapping top and bottom as the reference does. */
export function loraWallVerticalIndex(
  layout: LoraWallLayout,
  index: number,
  delta: -1 | 1,
): number {
  const box = layout.boxes[index];
  if (!box) return delta > 0 ? 0 : layout.boxes.length - 1;
  const target = requireValue(
    layout.rows[(box.row + delta + layout.rows.length) % layout.rows.length],
    'lora-wall-layout',
  );
  const centre = box.x + Math.floor((box.width - 1) / 2);
  let selected = target.from,
    distance = Infinity;
  for (let candidateIndex = target.from; candidateIndex < target.to; candidateIndex++) {
    const candidateBox = requireValue(layout.boxes[candidateIndex], 'lora-wall-layout');
    const nextDistance = Math.abs(
      candidateBox.x + Math.floor((candidateBox.width - 1) / 2) - centre,
    );
    if (nextDistance < distance) {
      distance = nextDistance;
      selected = candidateIndex;
    }
  }
  return selected;
}
/** Binary row lookup returns a contiguous visible/one-row-overscan tile interval without scanning unrelated candidates. */
export function loraWallVisibleRange(
  layout: LoraWallLayout,
  top: number,
  height: number,
): {
  readonly from: number;
  readonly to: number;
} {
  let low = 0,
    high = layout.rows.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    const row = requireValue(layout.rows[middle], 'lora-wall-layout');
    if (row.top + row.height < top) low = middle + 1;
    else high = middle;
  }
  const first = Math.max(0, low - 1);
  low = 0;
  high = layout.rows.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(layout.rows[middle], 'lora-wall-layout').top <= top + height) low = middle + 1;
    else high = middle;
  }
  const last = Math.min(layout.rows.length, low + 1);
  return {
    from: layout.rows[first]?.from ?? 0,
    to: layout.rows[last - 1]?.to ?? 0,
  };
}
