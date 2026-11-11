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
/** Actual proposed Scene context adapts Core semantics over the captured canonical rich syntax without another editor or map. */
import { requireValue } from '../../core/src/required.js';
import { Decoration, type DecorationSet } from '@codemirror/view';
import type { Range } from '@codemirror/state';
import { prepareSceneHeadings, type SourceRange } from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { compileSceneHeadingProjection, isSceneMarkerWidget } from './scene-projection.js';
import { projectOrdinaryReorderOutput } from './reorder-projection.js';
import {
  SourceWidget,
  EmphasisWidget,
  sourceDecorationToken,
  type SourceToken,
} from './projection-widgets.js';
const compiled = new WeakMap<ReorderPreview, DecorationSet>();
/** Compile only from the already prepared proposed SceneDocument; native measurement and readonly materialization never parse source. */
export function prepareReorderScenePaint(preview: ReorderPreview): DecorationSet {
  const previous = compiled.get(preview);
  if (previous) return previous;
  const ordinary = projectOrdinaryReorderOutput(preview.capture.projection, preview.output);
  // Ordinary paint already restores captured roles hidden by original headings; no proposed heading needs filtering or overlay.
  if (!preview.scenes?.scenes.length) {
    compiled.set(preview, ordinary);
    return ordinary;
  }
  const headings = prepareSceneHeadings(
    preview.scenes,
    requireValue(preview.capture.projection, 'reorder-scene-paint').sceneErrors.keys,
  );
  const scene = compileSceneHeadingProjection(
    headings,
    headings.flatMap(({ concealedRange, visibleRange }) => [
      concealedRange.from,
      visibleRange.from,
      visibleRange.to,
    ]),
  );
  /** Core emits exact paired bounds for every transparent shell occurrence, including reopened copies of the same original token. */
  const envelopes = new Map<string, SourceRange>();
  for (const occurrence of preview.output.origin.envelopes) {
    envelopes.set(
      `prefix:${occurrence.outerRange.from}:${occurrence.contentRange.from}`,
      occurrence.outerRange,
    );
    envelopes.set(
      `suffix:${occurrence.contentRange.to}:${occurrence.outerRange.to}`,
      occurrence.outerRange,
    );
  }
  /** Opaque emphasis and other atomic roles retain one canonical identity; transparent shell copies use their own Core occurrence instead. */
  const envelopeRange = (from: number, to: number, value: Decoration): SourceRange | undefined =>
    value.spec.widget instanceof EmphasisWidget
      ? envelopes.get(`${value.spec.widget.part}:${from}:${to}`)
      : undefined;
  /** Remaining single-occurrence roles include literal body text between their canonical wrappers. */
  const outer = new Map<
    SourceToken,
    {
      from: number;
      to: number;
    }
  >();
  ordinary.between(0, preview.output.text.length, (from, to, value) => {
    if (envelopeRange(from, to, value)) return;
    const token = sourceDecorationToken(value);
    if (!token) return;
    const previous = outer.get(token);
    if (previous) {
      previous.from = Math.min(previous.from, from);
      previous.to = Math.max(previous.to, to);
    } else outer.set(token, { from, to });
  });
  /** Ordered Core headings classify complete roles without recognizing syntax again. */
  const intersectsHeading = (from: number, to: number): boolean => {
    let low = 0,
      high = headings.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (requireValue(headings[middle], 'reorder-scene-paint').visibleRange.to <= from)
        low = middle + 1;
      else high = middle;
    }
    return (
      !!headings[low] && requireValue(headings[low], 'reorder-scene-paint').concealedRange.from < to
    );
  };
  const hidden = new Set(
    Array.from(outer, ([token, range]) =>
      intersectsHeading(range.from, range.to) ? token : null,
    ).filter((token): token is SourceToken => token !== null),
  );
  const decorations = ordinary.update({
    filter: (from, to, value) => {
      const occurrence = envelopeRange(from, to, value);
      if (occurrence) return !intersectsHeading(occurrence.from, occurrence.to);
      const token = sourceDecorationToken(value);
      return token ? !hidden.has(token) : !intersectsHeading(from, to);
    },
    add: scene.decorations,
    sort: true,
  });
  compiled.set(preview, decorations);
  return decorations;
}
/** Clip the one full prepared output into a canonical hard-line fragment; only empty Scene prefixes may split across literal leading-padding carriers, while painted/media widgets remain indivisible. */
export function clipReorderScenePaint(
  decorations: DecorationSet,
  range: SourceRange,
): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  decorations.between(range.from, range.to, (from, to, value) => {
    const start = Math.max(from, range.from),
      end = Math.min(to, range.to);
    if (start >= end) return;
    if (
      value.spec.widget instanceof SourceWidget &&
      !isSceneMarkerWidget(value.spec.widget) &&
      (start !== from || end !== to)
    )
      throw new Error('reorder-projection-split-widget');
    ranges.push(value.range(start - range.from, end - range.from));
  });
  return Decoration.set(ranges, true);
}
/** Binary lookup borrows the already ordered Core marker ranges; each hard-line fragment intersects at most one heading. All offsets are decoded output UTF-16. */
export function reorderSceneFragmentContext(
  preview: ReorderPreview,
  start: number,
  end: number,
): readonly (readonly [number, number, string, boolean])[] {
  const scenes = preview.scenes?.scenes;
  if (!scenes) return [];
  let low = 0,
    high = scenes.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(scenes[middle], 'reorder-scene-paint').marker.lineRange.to <= start)
      low = middle + 1;
    else high = middle;
  }
  const marker = scenes[low]?.marker;
  return marker && marker.lineRange.from < end
    ? [
        [
          Math.max(start, marker.lineRange.from) - start,
          Math.min(end, marker.lineRange.to) - start,
          marker.normalizedKey,
          marker.duplicate,
        ],
      ]
    : [];
}
