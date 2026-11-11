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
/** Prepared physical-line indexes own curly wildcard completion bounds and safe whole-placeholder edits. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { WildcardSpan } from '../prompt/contracts.js';
import { whitespace, caseFold } from '../prompt/text.js';
import { wildcardSpan } from '../prompt/wildcards.js';
import { assertCompletionBoundary } from './boundaries.js';
/** Current application completion uses curly wrappers; all ranges use exact-source UTF-16. */
export interface WildcardQuery {
  readonly prefix: string;
  readonly contentRange: SourceRange;
  readonly contentFieldRange: SourceRange;
  readonly replacementRange: SourceRange;
}
/** Immutable source indexes exclude unrelated lines and never parse source during native acceptance. */
export interface PreparedWildcardQueries {
  /** Reject invalid editable boundaries; selections and incompatible right content suppress completion. */
  queryAt(position: number, hasSelection: boolean): WildcardQuery | null;
}
const querySources = new WeakMap<WildcardQuery, string>();
/** Source transformations and previews accept only descriptors issued by their actual preparation owner. */
function assertPreparedQuery(source: string, query: WildcardQuery): void {
  if (querySources.get(query) !== source) throw new TypeError('Stale or unprepared wildcard query');
}
/** Find the first indexed source position at or after a UTF-16 boundary. */
function lowerBound(positions: readonly number[], position: number): number {
  let lower = 0,
    upper = positions.length;
  while (lower < upper) {
    const middle = (lower + upper) >>> 1;
    if (requireValue(positions[middle], 'wildcard-query') < position) lower = middle + 1;
    else upper = middle;
  }
  return lower;
}
/** Capture delimiter, physical-line and whitespace indexes once for one authoritative source lifetime. */
export function prepareWildcardQueries(source: string): PreparedWildcardQueries {
  if (typeof source !== 'string') throw new TypeError('Invalid wildcard completion source');
  const openers: number[] = [],
    closers: number[] = [],
    endings: number[] = [],
    invalid: number[] = [],
    nonblank: number[] = [];
  // Positions before the first opener cannot belong to any wildcard query.
  for (let position = source.indexOf('{'); position >= 0 && position < source.length; position++) {
    const character = requireValue(source[position], 'wildcard-query');
    if (character === '{') openers.push(position);
    if (character === '}') closers.push(position);
    if (character === '\r' || character === '\n') endings.push(position);
    const blank = whitespace.test(character);
    if (blank || '{},'.includes(character)) invalid.push(position);
    if (!blank) nonblank.push(position);
  }
  return Object.freeze({
    /** Binary lookups qualify the nearest opener and first closer without traversing unrelated source. */
    queryAt(position: number, hasSelection: boolean): WildcardQuery | null {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      const from = openers[lowerBound(openers, position) - 1];
      if (from === undefined) return null;
      const lineFrom = endings[lowerBound(endings, position) - 1] ?? -1;
      if (from <= lineFrom) return null;
      const contentFrom = from + 1,
        blocked = invalid[lowerBound(invalid, contentFrom)];
      if (blocked !== undefined && blocked < position) return null;
      const lineTo = endings[lowerBound(endings, position)] ?? source.length,
        closing = closers[lowerBound(closers, position)];
      const hasCloser = closing !== undefined && closing < lineTo;
      if (hasCloser) {
        const right = nonblank[lowerBound(nonblank, position)];
        if (right !== undefined && right < closing) return null;
      }
      const query: WildcardQuery = Object.freeze({
        prefix: source.slice(contentFrom, position),
        contentRange: Object.freeze({ from: contentFrom, to: position }),
        contentFieldRange: Object.freeze({
          from: contentFrom,
          to: hasCloser ? closing : position,
        }),
        replacementRange: Object.freeze({
          from,
          to: hasCloser ? closing + 1 : position,
        }),
      });
      querySources.set(query, source);
      return query;
    },
  });
}
/** A catalog suggestion must parse as one untagged wildcard of its declared source kind. */
export function isWildcardCompletionBody(body: string, form: WildcardSpan['form']): boolean {
  if (typeof body !== 'string' || body.length > 4096 || (form !== 'simple' && form !== 'csv'))
    return false;
  const text = `{${body}}`,
    span = wildcardSpan(text, 0, text.length, 1, text.length - 1, true);
  return !!span && span.tag === null && span.form === form;
}
/** Prepare a literal catalog body as one complete curly wildcard; destructive or group-bearing suggestions reject. */
export function wildcardReplacement(
  source: string,
  query: WildcardQuery,
  body: string,
  form: WildcardSpan['form'],
): {
  readonly range: SourceRange;
  readonly text: string;
} {
  assertPreparedQuery(source, query);
  if (!isWildcardCompletionBody(body, form))
    throw new RangeError('Wildcard completion must preserve one untagged placeholder');
  return Object.freeze({ range: query.replacementRange, text: `{${body}}` });
}
/** Preview a case-fold-compatible original scalar suffix, omitting an existing closer and partial expanded-scalar matches. */
export function wildcardGhostText(source: string, query: WildcardQuery, body: string): string {
  assertPreparedQuery(source, query);
  const prefix = caseFold(query.prefix);
  if (!caseFold(body).startsWith(prefix)) return '';
  let folded = 0,
    original = 0;
  for (const character of body) {
    if (folded === prefix.length) break;
    folded += caseFold(character).length;
    original += character.length;
    if (folded > prefix.length) return '';
  }
  const closer = query.replacementRange.to > query.contentFieldRange.to ? '' : '}';
  return body.slice(original) + closer;
}
