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
/** Canonical full-row presentation consumes existing Core topology and prepared source hard lines without another parser. */
import { requireValue } from '../../core/src/required.js';
import type { CanonicalRowPaint } from './canonical-row-paint.js';
import type { ReorderLayout, EmptyReorderPartition } from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import type { SourceLineOrigin } from './source-line-origins.js';
import { preparedRegionPaint } from './region-paint.js';
import { prepareSceneBands, sceneBandIntersectsLine } from './scene-line-bands.js';
/** Existing serialized chip/empty-target positions classify destination partitions; copied source marks cannot classify a moved chip's region. */
export function prepareReorderRowPaint(
  preview: ReorderPreview,
  lines: readonly SourceLineOrigin[],
  layout: ReorderLayout,
  emptyPartitions: readonly EmptyReorderPartition[],
): ReadonlyMap<SourceLineOrigin, CanonicalRowPaint> {
  const bands = prepareSceneBands(preview.scenes),
    paints =
      preview.capture.projection &&
      preview.capture.prepared.scope.container.location.kind === 'prompt'
        ? preparedRegionPaint(preview.capture.prepared.scope)
        : [];
  const result = new Map<SourceLineOrigin, CanonicalRowPaint>();
  if (!bands.length && !paints.length) return result;
  const partitions = new Map<SourceLineOrigin, number>();
  /** Binary lookup consumes the existing prepared hard-line index; no source scanning occurs here. */
  const first = (position: number): number => {
    let low = 0,
      high = lines.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (requireValue(lines[middle], 'reorder-row-paint').range.to < position) low = middle + 1;
      else high = middle;
    }
    return low;
  };
  /** All fragments of one output hard line share its actual destination partition, including multi-line chips and empty rows. */
  const assign = (from: number, to: number, partition: number): void => {
    for (
      let at = first(from);
      at < lines.length && requireValue(lines[at], 'reorder-row-paint').range.from <= to;
      at++
    ) {
      const line = requireValue(lines[at], 'reorder-row-paint'),
        existing = partitions.get(line);
      if (existing !== undefined && existing !== partition)
        throw new Error('canonical-row-partition-conflict');
      partitions.set(line, partition);
    }
  };
  if (paints.length) {
    for (const row of layout.rows)
      for (const id of row.chips) {
        const range = requireValue(preview.output.renderedRanges[id], 'reorder-row-paint');
        assign(range.from, range.to, row.partition);
      }
    /** Structural carrier starts come directly from the exact existing output ranges. */
    const carrierStart = (carrier: 'prefix' | 'slot' | 'suffix', slot: number): number =>
      carrier === 'prefix'
        ? 0
        : carrier === 'suffix'
          ? preview.output.text.length - preview.order.suffix.text.length
          : requireValue(preview.output.slotRanges[slot], 'reorder-row-paint').from;
    for (const gap of layout.gaps)
      for (const offset of gap.blankOffsets) {
        const position = carrierStart(gap.carrier, gap.slot) + offset;
        assign(position, position, gap.partition);
      }
    for (const empty of emptyPartitions) {
      const position = carrierStart(empty.carrier, empty.slot) + empty.offset;
      assign(position, position, empty.partition);
    }
  }
  for (const line of lines) {
    const sceneFill = sceneBandIntersectsLine(bands, line.range.from, line.range.to),
      partition = partitions.get(line),
      region = partition === undefined || partition === 0 ? null : (paints[partition - 1] ?? null);
    if (sceneFill || region) result.set(line, Object.freeze({ sceneFill, region }));
  }
  return result;
}
