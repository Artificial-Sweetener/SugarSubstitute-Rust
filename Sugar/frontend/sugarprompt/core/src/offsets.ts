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
/** Explicit Python code-point and UTF-16 boundary conversions; no editor or framework ownership. */
import { requireValue } from './required.js';
/** A half-open range measured in UTF-16 code units of exact source, including CRLF's two units. */
export interface SourceRange {
  readonly from: number;
  readonly to: number;
}
/** Build Python-index to UTF-16 boundaries, preserving unpaired surrogates as single code points. */
export function codePointBoundaries(source: string): readonly number[] {
  const boundaries = [0];
  for (const character of source)
    boundaries.push(requireValue(boundaries[boundaries.length - 1], 'offsets') + character.length);
  return boundaries;
}
/** Convert a Python code-point boundary; invalid indices reject instead of clamping. */
export function codePointToUtf16(source: string, position: number): number {
  const value = codePointBoundaries(source)[position];
  if (!Number.isSafeInteger(position) || value === undefined)
    throw new RangeError('Invalid code-point boundary');
  return value;
}
/** Convert a UTF-16 boundary; reject positions inside an astral code point explicitly. */
export function utf16ToCodePoint(source: string, position: number): number {
  const value = codePointBoundaries(source).indexOf(position);
  if (!Number.isSafeInteger(position) || value < 0)
    throw new RangeError('Invalid UTF-16 code-point boundary');
  return value;
}
