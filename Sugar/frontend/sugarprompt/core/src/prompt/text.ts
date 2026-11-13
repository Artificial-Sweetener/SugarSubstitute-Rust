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
/** Unicode text rules shared by prompt syntax; whitespace matches Python 3.12 rather than JS trim. */
import { requireValue } from '../required.js';
import { foldRanges, foldMappings } from './unicode-casefold-data.js';
import { unicodeAlphanumeric } from './alphanumeric.js';
/** Python's Unicode whitespace set, including information separators and excluding BOM. */
export const whitespace =
  // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
  /[\t-\r\x1c-\x20\x85\xa0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/u;
/** Remove only the reference's Unicode whitespace at the requested boundaries. */
export function strip(text: string, side: 'both' | 'left' | 'right' = 'both'): string {
  let from = 0,
    to = text.length;
  if (side !== 'right')
    while (from < to && whitespace.test(requireValue(text[from], 'text'))) from++;
  if (side !== 'left')
    while (to > from && whitespace.test(requireValue(text[to - 1], 'text'))) to--;
  return text.slice(from, to);
}
/** Fold semantic identities with Unicode 15.0 mappings, preserving characters unknown to that version. */
export function caseFold(text: string): string {
  // eslint-disable-next-line no-control-regex -- Preserve the exact ASCII fast path before Unicode folding.
  if (!/[^\x00-\x7f]/u.test(text)) return text.toLowerCase();
  let result = '';
  for (const character of text) {
    const point = requireValue(character.codePointAt(0), 'text');
    const mapping = foldMappings.get(point);
    if (mapping !== undefined) {
      result += mapping;
      continue;
    }
    const range = foldRanges.find(
      ([from, to, step]) => point >= from && point <= to && (point - from) % step === 0,
    );
    result += range ? String.fromCodePoint(point + range[3]) : character;
  }
  return result;
}
/** Freeze only derived semantic trees so consumers cannot mutate cached editor state. */
export function freezeStructure<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freezeStructure(child);
    Object.freeze(value);
  }
  return value;
}
/** Apostrophes between pinned Unicode 15 alphanumerics are filename or word text; exact bounded scalar neighbors preserve astral letters. */
export function isStructuralQuote(source: string, index: number): boolean {
  if (source[index] !== "'") return true;
  const previous = unicodeAlphanumeric(
    Array.from(source.slice(Math.max(0, index - 2), index)).at(-1) ?? '',
  );
  const next = unicodeAlphanumeric(Array.from(source.slice(index + 1, index + 3))[0] ?? '');
  return !(previous && next);
}
