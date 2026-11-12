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
/** One immutable canonical fragment plan supplies rendering and exact existing-paint admission. */
import { requireValue } from '../../core/src/required.js';
import type { DecorationSet } from '@codemirror/view';
import {
  prepareReorderScenePaint,
  clipReorderScenePaint,
  reorderSceneFragmentContext,
} from './reorder-scene-paint.js';
import { prepareReorderRowPaint } from './reorder-row-paint.js';
import type { CanonicalRowPaint } from './canonical-row-paint.js';
import { valueReorderLayout, emptyValueReorderPartitions } from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { preparedReorderSourceLines } from './source-line-origins.js';
import { reorderOriginFragments } from './reorder-origin-fragments.js';
import { prepareReorderStoragePaint, type ReorderStoragePaint } from './reorder-storage-paint.js';
/** Every range is in the Core publication's logical UTF-16 output coordinates. */
export interface ReorderPaintLine {
  readonly kind: 'line';
  readonly key: string;
  readonly identity: number;
  readonly from: number;
  readonly to: number;
  readonly pieces: ReturnType<typeof reorderOriginFragments>;
  /** Exact context-classified rich paint is prepared once, before any mounted resource admission. */
  readonly decorations: DecorationSet;
  /** Complete native line-edge resources paint prepared destination Scene fill and regional rails independently of source chip provenance. */
  readonly rowPaint: CanonicalRowPaint | null;
}
/** Literal stored carriers and complete canonical hard-line fragments share one ordered plan. */
export type ReorderPaintPart =
  | ReorderPaintLine
  | {
      readonly kind: 'text';
      readonly value: string;
    };
/** Empty partition/gap witnesses retain the canonical carrier split and target identity. */
export interface ReorderPaintMarker {
  readonly kind: 'partition' | 'gap';
  readonly index: number;
  readonly blank: number | null;
}
/** Each gap segment is painted before its optional empty target witness. */
export interface ReorderPaintSegment {
  readonly parts: readonly ReorderPaintPart[];
  readonly marker: ReorderPaintMarker | null;
}
/** Complete native inline wrappers are described without creating or cloning their DOM. */
export type ReorderPaintBlock =
  | {
      readonly kind: 'chip';
      readonly id: number;
      readonly parts: readonly ReorderPaintPart[];
    }
  | {
      readonly kind: 'gap';
      readonly segments: readonly ReorderPaintSegment[];
    };
/** Both cold paint and warm admission consume these exact fragment keys, including every duplicate occurrence. */
export interface ReorderWidgetPlan {
  readonly storage: ReorderStoragePaint;
  readonly blocks: readonly ReorderPaintBlock[];
  readonly keys: readonly string[];
  readonly empty: boolean;
  /** Full-row native Layer resources have no external whole-Stage painter. */
  readonly nativeRowPaint: boolean;
}
const plans = new WeakMap<ReorderPreview, ReorderWidgetPlan>();
/** Stage READ consumes only the existing immutable canonical plan; an unprepared publication cannot certify absent row resources. */
export function preparedReorderRowPaintAdmission(
  preview: ReorderPreview,
): 'unprepared' | 'present' | 'absent' {
  const plan = plans.get(preview);
  return !plan ? 'unprepared' : plan.nativeRowPaint ? 'present' : 'absent';
}
/** Produce canonical carriers, hard-line intersections and origin identities once; offsets remain logical output UTF-16. */
export function prepareReorderWidgetPlan(preview: ReorderPreview): ReorderWidgetPlan {
  const previous = plans.get(preview);
  if (previous) return previous;
  const scenePaint = prepareReorderScenePaint(preview);
  const { order, output } = preview,
    storage = prepareReorderStoragePaint(preview),
    lines = preparedReorderSourceLines(preview.capture, output),
    layout = valueReorderLayout(order),
    partitions = emptyValueReorderPartitions(preview.capture.prepared, order),
    keys: string[] = [],
    blocks: ReorderPaintBlock[] = [];
  const rowPaints = prepareReorderRowPaint(preview, lines, layout, partitions);
  /** Include literal endings and every complete canonical fragment with the renderer's exact original origin-key rule. */
  const fragment = (from: number, to: number): readonly ReorderPaintPart[] => {
    const parts: ReorderPaintPart[] = [];
    let low = 0,
      high = lines.length;
    while (low < high) {
      const mid = (low + high) >>> 1;
      if (requireValue(lines[mid], 'reorder-widget-plan').range.to < from) low = mid + 1;
      else high = mid;
    }
    const first = lines[low]?.range.from;
    if (first !== undefined && from < first)
      parts.push({
        kind: 'text',
        value: storage.literal({ from, to: Math.min(first, to) }),
      });
    for (
      let at = low;
      at < lines.length && requireValue(lines[at], 'reorder-widget-plan').range.from <= to;
      at++
    ) {
      const line = requireValue(lines[at], 'reorder-widget-plan'),
        start = Math.max(from, line.range.from),
        end = Math.min(to, line.range.to);
      if (start > end) continue;
      const pieces = reorderOriginFragments(output, { from: start, to: end });
      const rowPaint = rowPaints.get(line) ?? null;
      const context = reorderSceneFragmentContext(preview, start, end);
      const decorations = clipReorderScenePaint(scenePaint, {
          from: start,
          to: end,
        }),
        footprint: (readonly [number, number, boolean, boolean])[] = [];
      // A distant heading can suppress a whole wrapper role even when this fragment has identical origins and no local heading.
      decorations.between(0, end - start, (from, to, value) => {
        footprint.push([from, to, value.spec.scene === true, !!value.spec.widget]);
      });
      const key = JSON.stringify([
        footprint,
        rowPaint,
        context,
        pieces.map((piece) =>
          piece.kind === 'derived'
            ? [
                piece.reason,
                output.text.slice(start + piece.outputRange.from, start + piece.outputRange.to),
              ]
            : piece.kind === 'composed'
              ? null
              : [piece.sourceRange.from, piece.sourceRange.to],
        ),
      ]);
      keys.push(key);
      parts.push(
        Object.freeze({
          kind: 'line',
          key,
          identity: line.identity,
          from: start,
          to: end,
          pieces,
          decorations,
          rowPaint,
        }),
      );
      const next = lines[at + 1]?.range.from;
      if (next !== undefined && end < to && next > end)
        parts.push({
          kind: 'text',
          value: storage.literal({ from: end, to: Math.min(next, to) }),
        });
    }
    return Object.freeze(parts);
  };
  let cursor = 0;
  /** The canonical carrier policy emits every empty partition and blank-gap split before the trailing fragment. */
  const carrier = (to: number, owner: 'prefix' | 'slot' | 'suffix', slot: number): void => {
    const start = cursor,
      segments: ReorderPaintSegment[] = [];
    const markers = [
      ...partitions
        .filter((p) => p.carrier === owner && (owner !== 'slot' || p.slot === slot))
        .map((p) => ({
          offset: p.offset,
          marker: {
            kind: 'partition' as const,
            index: p.partition,
            blank: null,
          },
        })),
      ...layout.gaps
        .filter((gap) => gap.carrier === owner && (owner !== 'slot' || gap.slot === slot))
        .flatMap((gap) =>
          gap.blankOffsets.map((offset, blank) => ({
            offset,
            marker: { kind: 'gap' as const, index: gap.index, blank },
          })),
        ),
    ].sort((a, b) => a.offset - b.offset);
    let at = start;
    for (const { offset, marker } of markers) {
      const position = start + offset;
      if (position < at || position > to) continue;
      segments.push(
        Object.freeze({
          parts: fragment(at, position),
          marker: Object.freeze(marker),
        }),
      );
      at = position;
    }
    segments.push(Object.freeze({ parts: fragment(at, to), marker: null }));
    blocks.push(Object.freeze({ kind: 'gap', segments: Object.freeze(segments) }));
    cursor = to;
  };
  for (const [at, id] of order.order.entries()) {
    const range = requireValue(output.renderedRanges[id], 'reorder-widget-plan');
    carrier(range.from, at ? 'slot' : 'prefix', at - 1);
    blocks.push(
      Object.freeze({
        kind: 'chip',
        id,
        parts: fragment(range.from, range.to),
      }),
    );
    cursor = range.to;
  }
  carrier(output.text.length, 'suffix', -1);
  const plan = Object.freeze({
    storage,
    blocks: Object.freeze(blocks),
    keys: Object.freeze(keys),
    empty: !order.order.length && !partitions.length,
    nativeRowPaint: rowPaints.size > 0,
  });
  plans.set(preview, plan);
  return plan;
}
