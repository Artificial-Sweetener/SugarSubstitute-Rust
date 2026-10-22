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
/** Prepared physical-line scene slots own literal title replacement ranges independently of tag syntax. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { whitespace, strip } from '../prompt/text.js';
import { assertCompletionBoundary } from './boundaries.js';
/** Scene marker and replacement bounds use exact-source UTF-16; the prefix includes authored leading title whitespace. */
export interface SceneQuery {
  readonly prefix: string;
  readonly markerRange: SourceRange;
  readonly replacementRange: SourceRange;
}
/** One source lifetime indexes even unfinished markers; selection and nonblank right text suppress a title query. */
export interface PreparedSceneQueries {
  /** Validate scalar/CRLF boundaries and locate the current physical title slot without scanning unrelated lines. */
  queryAt(position: number, hasSelection: boolean): SceneQuery | null;
}
/** Source indexes retain only title bounds and the last nonblank boundary needed for caret lookup. */
interface SceneSlot {
  readonly markerFrom: number;
  readonly titleFrom: number;
  readonly to: number;
  readonly nonblankEnd: number;
}
const querySources = new WeakMap<SceneQuery, string>();
/** Capture horizontal-indented leading markers and physical CR/LF line boundaries outside native input. */
export function prepareSceneQueries(source: string): PreparedSceneQueries {
  if (typeof source !== 'string') throw new TypeError('Invalid scene query source');
  const slots: SceneSlot[] = [];
  for (let from = 0; from <= source.length; ) {
    let to = from;
    while (to < source.length && source[to] !== '\r' && source[to] !== '\n') to++;
    let markerFrom = from;
    while (markerFrom < to && /[ \t]/u.test(requireValue(source[markerFrom], 'scene-query')))
      markerFrom++;
    if (markerFrom + 2 <= to && source.slice(markerFrom, markerFrom + 2) === '**') {
      let nonblankEnd = to;
      while (
        nonblankEnd > markerFrom + 2 &&
        whitespace.test(requireValue(source[nonblankEnd - 1], 'scene-query'))
      )
        nonblankEnd--;
      slots.push({ markerFrom, titleFrom: markerFrom + 2, to, nonblankEnd });
    }
    if (to === source.length) break;
    from = to + (source[to] === '\r' && source[to + 1] === '\n' ? 2 : 1);
  }
  return Object.freeze({
    /** Binary lookup proves the complete right slice is blank before capturing one issued source query. */
    queryAt(position: number, hasSelection: boolean): SceneQuery | null {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      let lower = 0,
        upper = slots.length;
      while (lower < upper) {
        const middle = (lower + upper) >>> 1;
        if (requireValue(slots[middle], 'scene-query').titleFrom <= position) lower = middle + 1;
        else upper = middle;
      }
      const slot = slots[lower - 1];
      if (!slot || position > slot.to || position < slot.nonblankEnd) return null;
      const query: SceneQuery = Object.freeze({
        prefix: source.slice(slot.titleFrom, position),
        markerRange: Object.freeze({
          from: slot.markerFrom,
          to: slot.titleFrom,
        }),
        replacementRange: Object.freeze({ from: slot.titleFrom, to: position }),
      });
      querySources.set(query, source);
      return query;
    },
  });
}
/** A host title is finite literal single-line content; it cannot create another scene or control character. */
export function isSceneTitle(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length <= 4096 &&
    !!strip(value) &&
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    !/[\0\n-\r\x1c-\x1e\x85\u2028\u2029]/u.test(value)
  );
}
/** Accept exactly the host title, preserving marker, indentation and all right-hand whitespace and line endings. */
export function sceneReplacement(
  source: string,
  query: SceneQuery,
  title: string,
): {
  readonly range: SourceRange;
  readonly text: string;
} {
  if (querySources.get(query) !== source) throw new TypeError('Stale or unprepared scene query');
  if (!isSceneTitle(title)) throw new TypeError('Invalid scene title');
  return Object.freeze({ range: query.replacementRange, text: title });
}
