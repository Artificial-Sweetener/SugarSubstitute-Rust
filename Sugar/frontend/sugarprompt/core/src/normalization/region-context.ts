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
/** Regional line conventions use bounded Unicode-scalar context with exact-source UTF-16 positions. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
/** Recognize odd backslash escape parity at one source character. */
export function isEscaped(source: string, position: number): boolean {
  let start = position;
  while (start > 0 && source[start - 1] === '\\') start--;
  return (position - start) % 2 === 1;
}
/** Find the beginning of the source line before an exclusive UTF-16 boundary. */
export function sourceLineStart(source: string, position: number): number {
  if (position <= 0) return 0;
  return (
    Math.max(source.lastIndexOf('\r', position - 1), source.lastIndexOf('\n', position - 1)) + 1
  );
}
/** Find a CR/LF boundary without treating other Unicode whitespace as a hard line. */
function nextBreak(source: string, from: number): number | null {
  const values = [source.indexOf('\r', from), source.indexOf('\n', from)].filter(
    (position) => position >= 0,
  );
  return values.length ? Math.min(...values) : null;
}
/** Include the containing and following source lines for local separator deletion repair. */
export function separatorWindow(source: string, position: number): SourceRange {
  const from = sourceLineStart(source, position),
    first = nextBreak(source, position);
  if (first === null) return { from, to: source.length };
  const next = first + (source.startsWith('\r\n', first) ? 2 : 1);
  return { from, to: nextBreak(source, next) ?? source.length };
}
/** Move at most 512 scalar boundaries without scanning or allocating the rest of the document. */
function contextBoundary(source: string, position: number, forward: boolean): number {
  for (let count = 0; count < 512 && (forward ? position < source.length : position > 0); count++) {
    if (forward)
      position += requireValue(source.codePointAt(position), 'region-context') > 0xffff ? 2 : 1;
    else {
      position--;
      if (
        position > 0 &&
        /[\udc00-\udfff]/u.test(requireValue(source[position], 'region-context')) &&
        /[\ud800-\udbff]/u.test(requireValue(source[position - 1], 'region-context'))
      )
        position--;
    }
  }
  return position;
}
/** Prefer the closest original newline by scalar distance to marker start, preserving earlier ties. */
export function preferredRegionEnding(source: string, range: SourceRange): string {
  const from = contextBoundary(source, range.from, false),
    to = contextBoundary(source, range.to, true);
  const context = [...Array.from(source.slice(from, to))],
    marker = [...Array.from(source.slice(from, range.from))].length;
  let ending = '\n',
    distance = Infinity;
  for (let index = 0; index < context.length; index++) {
    const character = context[index];
    if (character !== '\r' && character !== '\n') continue;
    const candidate = character === '\r' && context[index + 1] === '\n' ? '\r\n' : character;
    if (Math.abs(index - marker) < distance) {
      ending = candidate;
      distance = Math.abs(index - marker);
    }
    if (candidate === '\r\n') index++;
  }
  return ending;
}
