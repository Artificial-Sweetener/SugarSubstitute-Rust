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
/** Literal tag acceptance, no-op comparison and inline suffix semantics share one source transformation owner. */
import { requireValue } from '../required.js';
import { normalizeTagIdentity } from '../normalization/lexicon.js';
import { mapVisibleSource } from '../prompt/display.js';
import type { TagQuery, TagReplacement } from './contracts.js';
import { assertTagQuery } from './boundaries.js';
/** Format ordinary tag source: underscores become spaces and parentheses become literal stored escapes. */
export function tagReplacementText(tag: string): string {
  return tag.replaceAll('_', ' ').replace(/[()]/gu, '\\$&');
}
/** Compare case-sensitive scalar characters, with spaces/underscores equivalent as in the pinned inline display. */
function charactersMatch(left: string, right: string): boolean {
  return left === right || (left === '_' && right === ' ') || (left === ' ' && right === '_');
}
/** Return only a compatible visible suffix; case-folded catalog matching does not force a case-changing ghost. */
export function completionSuffix(suggestion: string, prefix: string): string {
  const proposed = Array.from(suggestion),
    count = matchingPrefixLength(proposed, prefix);
  return count < 0 ? '' : proposed.slice(count).join('');
}
/** One scalar comparison owner keeps suffix projection and prepared acceptance case-sensitive with space/underscore equivalence. */
function matchingPrefixLength(proposed: readonly string[], prefix: string): number {
  const typed = Array.from(prefix);
  return typed.length > proposed.length ||
    typed.some(
      (character, index) => !charactersMatch(character, requireValue(proposed[index], 'text')),
    )
    ? -1
    : typed.length;
}
/** Omit a compatible tail already right of the caret; only trailing ASCII space/tab is ignored. */
export function suffixWithoutRightText(suffix: string, rightText: string): string {
  const right = Array.from(rightText.replace(/[ \t]+$/u, '')),
    proposed = Array.from(suffix);
  if (!right.length || right.length > proposed.length) return suffix;
  const start = proposed.length - right.length;
  return right.every((character, index) =>
    charactersMatch(character, requireValue(proposed[start + index], 'text')),
  )
    ? proposed.slice(0, start).join('')
    : suffix;
}
/** Inspect already-matching source through literal escape/Unicode identity without changing source. */
export function tagIsNoop(source: string, query: TagQuery, tag: string): boolean {
  assertTagQuery(source, query);
  return (
    normalizeTagIdentity(
      mapVisibleSource(source.slice(query.replacementRange.from, query.activeTagEnd)).text,
    ) === normalizeTagIdentity(mapVisibleSource(tagReplacementText(tag)).text)
  );
}
/** Prepare inline display from source/query metadata; the caller owns source freshness and ghost permission. */
export function tagGhostText(source: string, query: TagQuery, tag: string): string {
  assertTagQuery(source, query);
  const suffix = completionSuffix(mapVisibleSource(tagReplacementText(tag)).text, query.prefix);
  return suffixWithoutRightText(
    suffix,
    mapVisibleSource(source.slice(query.replacementRange.to, query.activeTagEnd)).text,
  );
}
/** Propose one exact-source acceptance, consuming compatible right text and retaining unmatched right text. */
export function tagReplacement(
  source: string,
  query: TagQuery,
  tag: string,
  addComma: boolean,
): TagReplacement {
  const prepared = prepareTagReplacement(source, query, tag, addComma);
  return prepared.right && prepared.right.match.consumesVisiblePrefix(query.prefix)
    ? prepared.right.replacement
    : prepared.retainRight;
}
/** Prepared right-text matching reads only a candidate-bounded prefix; it contains no source buffer or catalog work. */
export interface PreparedTagRightMatch {
  readonly maximumSourcePrefixLength: number;
  /** Compare an already visible prefix using the same scalar rules as ordinary acceptance. */
  consumesVisiblePrefix(prefix: string): boolean;
  /** Decode a bounded stored-source prefix before delegating to the shared comparison. */
  consumesSourcePrefix(prefix: string): boolean;
}
/** Both exact-source UTF-16 edits are prepared before input; the current prefix selects their immutable output. */
export interface PreparedTagReplacement {
  readonly retainRight: TagReplacement;
  readonly right: {
    readonly replacement: TagReplacement;
    readonly match: PreparedTagRightMatch;
  } | null;
}
/** Prepare one exact edit; whole-tag keyboard acceptance reuses an adjacent authored comma and horizontal whitespace. */
function continuationReplacement(
  source: string,
  query: TagQuery,
  replacementText: string,
  to: number,
  addComma: boolean,
): TagReplacement {
  let end = to;
  let continuation = addComma ? ', ' : '';
  if (addComma && to === query.activeTagEnd) {
    let comma = to;
    while (source[comma] === ' ' || source[comma] === '\t') comma++;
    if (source[comma] === ',') {
      end = comma + 1;
      while (source[end] === ' ' || source[end] === '\t') end++;
      continuation = source.slice(to, end);
    }
  }
  return Object.freeze({
    range: Object.freeze({ from: query.replacementRange.from, to: end }),
    text: replacementText + continuation,
  });
}
/** Prepare suffix-sensitive alternatives and their complete continuation edits before native input. */
export function prepareTagReplacement(
  source: string,
  query: TagQuery,
  tag: string,
  addComma: boolean,
): PreparedTagReplacement {
  assertTagQuery(source, query);
  const replacementText = tagReplacementText(tag);
  const suggestion = mapVisibleSource(replacementText).text;
  const right = mapVisibleSource(source.slice(query.replacementRange.to, query.activeTagEnd)).text;
  const beforeRight = suffixWithoutRightText(suggestion, right);
  const retainRight = continuationReplacement(
    source,
    query,
    replacementText,
    query.replacementRange.to,
    addComma,
  );
  if (beforeRight === suggestion) return Object.freeze({ retainRight, right: null });
  const proposed = Object.freeze(Array.from(beforeRight));
  const maximumSourcePrefixLength = beforeRight.length * 2;
  /** A current prefix consumes the fixed right tail only while it fits before that tail. */
  function consumesVisiblePrefix(prefix: string): boolean {
    return matchingPrefixLength(proposed, prefix) >= 0;
  }
  return Object.freeze({
    retainRight,
    right: Object.freeze({
      replacement: continuationReplacement(
        source,
        query,
        replacementText,
        query.activeTagEnd,
        addComma,
      ),
      match: Object.freeze({
        maximumSourcePrefixLength,
        consumesVisiblePrefix,
        /** Reject oversized input before any visible-source decoding. */
        consumesSourcePrefix(prefix: string): boolean {
          return (
            prefix.length <= maximumSourcePrefixLength &&
            consumesVisiblePrefix(mapVisibleSource(prefix).text)
          );
        },
      }),
    }),
  });
}
