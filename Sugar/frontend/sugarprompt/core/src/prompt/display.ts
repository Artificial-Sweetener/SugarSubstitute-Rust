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
/** Pure stored-source escape recognition; native editors own its visual projection and caret mapping. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
/** Escape-only display mapping; parallel arrays contain scalar boundaries in exact-source and display UTF-16. */
export interface VisibleSource {
  readonly text: string;
  readonly sourcePositions: readonly number[];
  readonly displayPositions: readonly number[];
}
/** Map literal parenthesis escapes through the same recognition owner used by native projection. */
export function mapVisibleSource(source: string, sourceStart = 0): VisibleSource {
  if (!Number.isSafeInteger(sourceStart) || sourceStart < 0)
    throw new RangeError('Invalid visible-source origin');
  const escapes = parenthesisEscapes(source);
  const sourcePositions = [sourceStart],
    displayPositions = [0],
    characters: string[] = [];
  let escapeIndex = 0,
    position = 0,
    displayPosition = 0;
  while (position < source.length) {
    const escaped = escapes[escapeIndex]?.from === position;
    const character = escaped
      ? requireValue(source[position + 1], 'display')
      : String.fromCodePoint(requireValue(source.codePointAt(position), 'display'));
    position += escaped ? 2 : character.length;
    if (escaped) escapeIndex++;
    displayPosition += character.length;
    characters.push(character);
    sourcePositions.push(sourceStart + position);
    displayPositions.push(displayPosition);
  }
  return Object.freeze({
    text: characters.join(''),
    sourcePositions: Object.freeze(sourcePositions),
    displayPositions: Object.freeze(displayPositions),
  });
}
/** Locate storage-only backslash/parenthesis pairs in exact-source UTF-16, including CRLF units. */
export function parenthesisEscapes(source: string): readonly SourceRange[] {
  const ranges: SourceRange[] = [];
  for (let index = 0; index < source.length - 1; index++) {
    if (source[index] === '\\' && (source[index + 1] === '(' || source[index + 1] === ')')) {
      ranges.push({ from: index, to: index + 2 });
      index++;
    }
  }
  return ranges;
}
