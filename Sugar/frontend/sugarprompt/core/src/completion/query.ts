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
/** Prepared source queries index segments, hard lines and nested emphasis before interactive lookup. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { PromptDocument, EmphasisSpan } from '../prompt/contracts.js';
import { mapVisibleSource, type VisibleSource } from '../prompt/display.js';
import { segmentSelectionRange } from '../prompt/selection.js';
import { whitespace } from '../prompt/text.js';
import type { PreparedTagQueries, TagQuery } from './contracts.js';
import { assertCompletionBoundary } from './boundaries.js';
/** One sorted emphasis entry links to its containing span, avoiding an unrelated-span scan. */
interface IndexedEmphasis {
  readonly span: EmphasisSpan;
  readonly parent: number;
}
/** Locate the last sorted entry starting at/before one exact-source position. */
function floorIndex<T>(
  entries: readonly T[],
  position: number,
  start: (entry: T) => number,
): number {
  let lower = 0,
    upper = entries.length;
  while (lower < upper) {
    const middle = (lower + upper) >>> 1;
    if (start(requireValue(entries[middle], 'query')) <= position) lower = middle + 1;
    else upper = middle;
  }
  return lower - 1;
}
/** Build parent links over canonical sorted spans once at document preparation. */
function indexEmphasis(spans: readonly EmphasisSpan[]): readonly IndexedEmphasis[] {
  const indexed: IndexedEmphasis[] = [],
    stack: number[] = [];
  for (const span of spans) {
    while (
      stack.length &&
      requireValue(indexed[requireValue(stack.at(-1), 'query')], 'query').span.outerRange.to <
        span.outerRange.to
    )
      stack.pop();
    indexed.push({ span, parent: stack.at(-1) ?? -1 });
    stack.push(indexed.length - 1);
  }
  return indexed;
}
/** Prepare whole-source hard-line ranges; CRLF remains two source units. */
function lineRanges(source: string): readonly SourceRange[] {
  const ranges: SourceRange[] = [];
  let from = 0;
  for (let index = 0; index < source.length; index++) {
    if (source[index] !== '\r' && source[index] !== '\n') continue;
    ranges.push({ from, to: index });
    if (source[index] === '\r' && source[index + 1] === '\n') index++;
    from = index + 1;
  }
  ranges.push({ from, to: source.length });
  return ranges;
}
/** Return the ordinal only at an exact mapped source boundary, excluding hidden escape interiors. */
function visibleIndex(visible: VisibleSource, position: number): number {
  const index = floorIndex(visible.sourcePositions, position, (value) => value);
  return visible.sourcePositions[index] === position ? index : -1;
}
/** Exclude authored boundary whitespace from replacement and bound the visible last-word fallback. */
function queryForRange(
  source: string,
  visible: VisibleSource,
  range: SourceRange,
  cursor: number,
  minimum: number,
): TagQuery | null {
  let from = range.from,
    to = range.to;
  while (from < to && whitespace.test(requireValue(source[from], 'query'))) from++;
  while (to > from && whitespace.test(requireValue(source[to - 1], 'query'))) to--;
  if (cursor < from) return null;
  range = { from, to: Math.max(cursor, to) };
  const start = visibleIndex(visible, range.from),
    caret = visibleIndex(visible, cursor);
  if (start < 0 || caret < start || caret - start < minimum) return null;
  const displayStart = requireValue(visible.displayPositions[start], 'query');
  const prefix = visible.text.slice(displayStart, visible.displayPositions[caret]);
  let suffixStart = 0;
  for (let index = 0; index < prefix.length; index++)
    if (whitespace.test(requireValue(prefix[index], 'query'))) suffixStart = index + 1;
  const suffix = prefix.slice(suffixStart);
  let fallback: TagQuery | null = null;
  if (suffixStart > 0 && Array.from(suffix).length >= minimum) {
    let localEnd = cursor;
    while (
      localEnd < range.to &&
      source[localEnd] !== ',' &&
      !whitespace.test(requireValue(source[localEnd], 'query'))
    )
      localEnd++;
    const suffixOrdinal = start + Array.from(prefix.slice(0, suffixStart)).length;
    fallback = Object.freeze({
      prefix: suffix,
      replacementRange: Object.freeze({
        from: requireValue(visible.sourcePositions[suffixOrdinal], 'query'),
        to: cursor,
      }),
      activeTagEnd: localEnd,
      fallback: null,
    });
  }
  return Object.freeze({
    prefix,
    replacementRange: Object.freeze({ from: range.from, to: cursor }),
    activeTagEnd: range.to,
    fallback,
  });
}
/** Prepare ordinary prompt query semantics from the authoritative parsed document without another parser/buffer owner. */
export function prepareTagQueries(document: PromptDocument): PreparedTagQueries {
  const source = document.sourceText;
  const segments = document.segments.map(segmentSelectionRange);
  const emphasis = indexEmphasis(document.emphasisSpans);
  const lines = lineRanges(source),
    visible = mapVisibleSource(source);
  return Object.freeze({
    /** Resolve scalar-safe source context; selection and insufficient visible prefix suppress suggestions. */
    queryAt(position: number, hasSelection: boolean, minimumPrefixLength = 2): TagQuery | null {
      assertCompletionBoundary(source, position);
      if (!Number.isSafeInteger(minimumPrefixLength) || minimumPrefixLength < 1)
        throw new RangeError('Invalid completion prefix length');
      if (hasSelection) return null;
      const segment = segments[floorIndex(segments, position, (range) => range.from)];
      if (!segment || position > segment.to) return null;
      let spanIndex = floorIndex(emphasis, position, (entry) => entry.span.outerRange.from);
      while (spanIndex >= 0) {
        const entry = requireValue(emphasis[spanIndex], 'query');
        const outer = entry.span.outerRange;
        if (position <= outer.to && outer.from >= segment.from && outer.to <= segment.to) {
          const content = entry.span.contentRange;
          return position < content.from || position > content.to
            ? null
            : queryForRange(source, visible, content, position, minimumPrefixLength);
        }
        spanIndex = entry.parent;
      }
      const line = requireValue(lines[floorIndex(lines, position, (range) => range.from)], 'query');
      return queryForRange(
        source,
        visible,
        { from: Math.max(segment.from, line.from), to: Math.min(segment.to, line.to) },
        position,
        minimumPrefixLength,
      );
    },
  });
}
