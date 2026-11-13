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
/** Regional normalization delegates lexical grammar and owns minimum line insertion/caret policies. */
import { requireValue } from '../required.js';
import { regionSeparatorAt, regionTokenAt, regionTokensInRange } from '../prompt/regions.js';
import type { RegionToken } from '../prompt/contracts.js';
import type { SourceRange } from '../offsets.js';
import { validateSourceRange } from './ranges.js';
import {
  prepareInsertions,
  type InsertionPreparation,
  type SourceInsertion,
} from './insertions.js';
import type { SourceNormalization } from './contracts.js';
import {
  isEscaped,
  preferredRegionEnding,
  separatorWindow,
  sourceLineStart,
} from './region-context.js';
/** Insert shared boundary newlines once, including adjacent markers with a single line between them. */
function prepareMarkers(
  text: string,
  tokens: readonly RegionToken[],
  trailing: boolean,
  beforeBoundary?: number,
): InsertionPreparation {
  const insertions = new Map<number, SourceInsertion>();
  for (const token of tokens) {
    const range = token.tokenRange;
    if (isEscaped(text, range.from)) continue;
    const ending = preferredRegionEnding(text, range);
    if (range.from > 0 && !/[\r\n]/u.test(requireValue(text[range.from - 1], 'regions')))
      if (!insertions.has(range.from))
        insertions.set(range.from, {
          position: range.from,
          text: ending,
          affinity: range.from === beforeBoundary ? 'before' : 'after',
        });
    if (
      (range.to === text.length && trailing) ||
      (range.to < text.length && !/[\r\n]/u.test(requireValue(text[range.to], 'regions')))
    )
      if (!insertions.has(range.to))
        insertions.set(range.to, {
          position: range.to,
          text: ending,
          affinity: range.to === beforeBoundary ? 'before' : 'after',
        });
  }
  return prepareInsertions(text, [...insertions.values()]);
}
/** Normalize complete unescaped markers contained by one post-paste exact-source UTF-16 range. */
export function preparePastedRegions(text: string, range: SourceRange): InsertionPreparation {
  validateSourceRange(text, range);
  return prepareMarkers(text, regionTokensInRange(text, range.from, range.to), false);
}
/** Keep content inserted into an empty partition separate from its following canonical marker. */
export function prepareEmptyRegionInsertion(
  text: string,
  range: SourceRange,
): InsertionPreparation {
  validateSourceRange(text, range);
  if (range.from < range.to) {
    for (const ending of ['\r\n', '\n', '\r']) {
      const tokenEnd = range.from - ending.length;
      if (tokenEnd < 0 || text.slice(tokenEnd, range.from) !== ending) continue;
      const previous = regionSeparatorAt(text, sourceLineStart(text, tokenEnd)),
        next = regionTokenAt(text, range.to);
      if (
        previous?.tokenRange.to === tokenEnd &&
        next &&
        (next.tokenRange.to === text.length ||
          /[\r\n]/u.test(requireValue(text[next.tokenRange.to], 'regions'))) &&
        !isEscaped(text, range.to)
      )
        return prepareInsertions(text, [{ position: range.to, text: ending, affinity: 'before' }]);
    }
  }
  return prepareInsertions(text, []);
}
/** Prepare completed markers in one typed exact-source UTF-16 range, including existing newline spelling after its terminal marker. */
export function prepareTypedRegions(
  text: string,
  range: SourceRange,
  replacement: string,
): InsertionPreparation {
  validateSourceRange(text, range);
  let token: RegionToken | null = null;
  if (replacement === ']' && range.to - range.from === 1) {
    const start = text.lastIndexOf('[SEP', range.to - 1);
    if (start >= sourceLineStart(text, range.to - 1)) token = regionTokenAt(text, start);
  }
  const tokens =
    replacement.length > 1 && range.to - range.from > 1
      ? regionTokensInRange(text, sourceLineStart(text, range.from), range.to).filter(
          (item) => item.tokenRange.to > range.from,
        )
      : token && token.tokenRange.to === range.to && !isEscaped(text, token.tokenRange.from)
        ? [token]
        : [];
  if (!tokens.length) return prepareInsertions(text, []);
  const prepared = prepareMarkers(text, tokens, true),
    result = prepared.normalization;
  if (!tokens.some((item) => item.tokenRange.to === range.to)) return prepared;
  const after =
      range.to +
      (text.startsWith('\r\n', range.to) ? 2 : /[\r\n]/u.test(text[range.to] ?? '') ? 1 : 0),
    boundaries = [...result.boundaryPositions];
  for (let position = range.to; position <= after; position++)
    boundaries[position] = requireValue(result.boundaryPositions[after], 'regions');
  return Object.freeze({
    ...prepared,
    normalization: Object.freeze({
      ...result,
      boundaryPositions: Object.freeze(boundaries),
    }),
  });
}
/** Restore complete markers around a deletion without normalizing unrelated source lines. */
export function normalizeTypedDeletion(text: string, position: number): SourceNormalization {
  validateSourceRange(text, { from: position, to: position });
  const window = separatorWindow(text, position);
  const beforeBoundary =
    regionTokenAt(text, position) &&
    position > 0 &&
    !/[\r\n]/u.test(requireValue(text[position - 1], 'regions'))
      ? position
      : undefined;
  return prepareMarkers(
    text,
    regionTokensInRange(text, window.from, window.to),
    false,
    beforeBoundary,
  ).normalization;
}
