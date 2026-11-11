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
/** Prompt pointer-selection semantics use exact-source UTF-16 boundaries. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { PromptSegment } from './contracts.js';
import { unicodeAlphanumeric } from './alphanumeric.js';
/** Preserve trailing whitespace and line endings; only leading ASCII space/tab is omitted by the donor's segment convention. */
export function segmentSelectionRange(segment: PromptSegment): SourceRange {
  const leading = requireValue(/^[ \t]*/u.exec(segment.text), 'selection')[0].length;
  return {
    from: Math.min(segment.contentRange.to, segment.contentRange.from + leading),
    to: segment.contentRange.to,
  };
}
/** Return the previous Unicode scalar's UTF-16 boundary, retaining isolated surrogate units. */
function previousBoundary(source: string, position: number): number {
  const previous = position - 1;
  return previous > 0 &&
    /[\uDC00-\uDFFF]/u.test(requireValue(source[previous], 'selection')) &&
    /[\uD800-\uDBFF]/u.test(requireValue(source[previous - 1], 'selection'))
    ? previous - 1
    : previous;
}
/** Extend pinned alphanumeric/underscore/hyphen selection with native base-associated combining marks. */
function wordCharacter(character: string): boolean {
  return unicodeAlphanumeric(character) || /^[\p{M}_-]$/u.test(character);
}
/** Resolve a prompt word without scanning unrelated source; surrogate-interior and invalid offsets reject explicitly. */
export function promptWordRange(source: string, position: number): SourceRange {
  if (
    !Number.isSafeInteger(position) ||
    position < 0 ||
    position > source.length ||
    (position > 0 &&
      position < source.length &&
      /[\uD800-\uDBFF]/u.test(requireValue(source[position - 1], 'selection')) &&
      /[\uDC00-\uDFFF]/u.test(requireValue(source[position], 'selection')))
  )
    throw new RangeError('Invalid prompt-word UTF-16 boundary');
  let from = position,
    to = position;
  while (from > 0) {
    const previous = previousBoundary(source, from);
    if (!wordCharacter(source.slice(previous, from))) break;
    from = previous;
  }
  while (to < source.length) {
    const character = String.fromCodePoint(requireValue(source.codePointAt(to), 'selection'));
    if (!wordCharacter(character)) break;
    to += character.length;
  }
  while (from < to) {
    const character = String.fromCodePoint(requireValue(source.codePointAt(from), 'selection'));
    if (!/^\p{M}$/u.test(character)) break;
    from += character.length;
  }
  if (position < from) return { from: position, to: position };
  return { from, to };
}
