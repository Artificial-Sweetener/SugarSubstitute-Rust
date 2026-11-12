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
/** Conservative content classification permits prepared replacement reuse without discovering new syntax during input. */
import { mapVisibleSource, parenthesisEscapes } from '../prompt/display.js';
import { loraNameContent } from './lora-text.js';
import { isStructuralQuote, whitespace } from '../prompt/text.js';

/** Content excludes mode delimiters; the native owner separately validates the query's bounds and trimmed start. */
export function stableCompletionContent(
  kind: 'tag' | 'lora' | 'wildcard' | 'scene',
  text: string,
): boolean {
  if (kind === 'scene')
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    return !/[\0\n-\r\x1c-\x1e\x85\u2028\u2029]/u.test(text);
  if (kind === 'wildcard') return !/[{},]/u.test(text) && !whitespace.test(text);
  return kind === 'lora'
    ? loraNameContent(text)
    : !/[\\()[\]{}<>,:'"\s\0]/u.test(text.replace(/[ \t]/gu, ''));
}

/** Fixed quotes and word separators remain inside the complete prepared replacement; their insertion/removal is checked separately. */
function stableTagText(text: string): boolean {
  return stableCompletionContent('tag', text.replace(/['"]/gu, ''));
}

/** Existing quotes and parenthesis escapes can remain fixed while ordinary content and word separators are refined. */
export function stableTagCompletionPrefix(text: string): boolean {
  let from = 0;
  for (const escape of parenthesisEscapes(text)) {
    if (!stableTagText(text.slice(from, escape.from))) return false;
    from = escape.to;
  }
  return stableTagText(text.slice(from));
}

/** Changed content must preserve escape and neighboring apostrophe meaning; three UTF-16 units on either side cover a quote plus an astral neighbor. */
export function stableCompletionEdit(
  kind: 'tag' | 'lora' | 'wildcard' | 'scene',
  before: string,
  removed: string,
  inserted: string,
  after: string,
): boolean {
  if (!stableCompletionContent(kind, removed) || !stableCompletionContent(kind, inserted))
    return false;
  if (kind !== 'tag') return true;
  const left = before.slice(-3),
    right = after.slice(0, 3);
  if (parenthesisEscapes(left.slice(-1) + (removed[0] ?? right[0] ?? '')).length) return false;
  if (
    left.endsWith("'") &&
    isStructuralQuote(left + removed.slice(0, 2) + right, left.length - 1) !==
      isStructuralQuote(left + inserted.slice(0, 2) + right, left.length - 1)
  )
    return false;
  const oldBefore = left + removed.slice(-2),
    newBefore = left + inserted.slice(-2);
  return (
    !right.startsWith("'") ||
    isStructuralQuote(oldBefore + right, oldBefore.length) ===
      isStructuralQuote(newBefore + right, newBefore.length)
  );
}

/** At most four source code units contain the first two visible scalars, including literal escapes and astral text. */
export function hasCompletionPrefix(sourcePrefix: string): boolean {
  return Array.from(mapVisibleSource(sourcePrefix.slice(0, 4)).text).length >= 2;
}
