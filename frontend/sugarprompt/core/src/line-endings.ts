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
/** Exact newline spelling separate from logical editor text, never a second mutable source document. */
import { requireValue } from './required.js';
import type { SourceRange } from './offsets.js';
/** An original newline spelling; each represents one logical CodeMirror newline. */
export type LineEnding = '\n' | '\r' | '\r\n';
/** Temporary conversion result used to initialize or insert into the authoritative document. */
export interface EncodedSource {
  readonly logical: string;
  readonly endings: readonly LineEnding[];
}
/** Split arbitrary original line endings into logical LF and their exact ordered spellings. */
export function encodeSource(source: string): EncodedSource {
  const endings: LineEnding[] = [];
  const logical = source.replace(/\r\n|\r|\n/gu, (ending) => {
    endings.push(ending as LineEnding);
    return '\n';
  });
  return { logical, endings };
}
/** Serialize document text with its newline metadata, rejecting inconsistent state. */
export function decodeSource(logical: string, endings: readonly LineEnding[]): string {
  let index = 0;
  const source = logical.replace(/\n/gu, () => {
    const ending = endings[index++];
    if (ending === undefined) throw new Error('Missing line-ending metadata');
    return ending;
  });
  if (index !== endings.length || logical.includes('\r'))
    throw new Error('Invalid logical source representation');
  return source;
}
/** Convert logical document UTF-16 to exact source UTF-16, accounting for CRLF expansion. */
export function documentToSource(
  logical: string,
  endings: readonly LineEnding[],
  position: number,
): number {
  if (!Number.isSafeInteger(position) || position < 0 || position > logical.length)
    throw new RangeError('Invalid document position');
  let extra = 0,
    index = 0;
  for (let cursor = 0; cursor < position; cursor++)
    if (logical[cursor] === '\n') extra += (endings[index++]?.length ?? 1) - 1;
  return position + extra;
}
/** Convert exact source UTF-16 to logical document UTF-16; reject the interior of a CRLF pair. */
export function sourceToDocument(
  logical: string,
  endings: readonly LineEnding[],
  position: number,
): number {
  return requireValue(sourcePositionsToDocument(logical, endings, [position])[0], 'line-endings');
}
/** Convert exact-source UTF-16 boundaries in one document traversal, preserving input order and duplicates. */
export function sourcePositionsToDocument(
  logical: string,
  endings: readonly LineEnding[],
  positions: readonly number[],
): readonly number[] {
  if (!endings.includes('\r\n'))
    return positions.map((position) => {
      if (!Number.isSafeInteger(position) || position < 0 || position > logical.length)
        throw new RangeError('Invalid source position');
      return position;
    });
  const ordered = positions
    .map((position, index) => {
      if (!Number.isSafeInteger(position) || position < 0)
        throw new RangeError('Invalid source position');
      return { position, index };
    })
    .sort((left, right) => left.position - right.position);
  const converted: number[] = new Array(positions.length);
  let source = 0,
    cursor = 0,
    newline = 0;
  for (const boundary of ordered) {
    while (source < boundary.position && cursor < logical.length) {
      source += logical[cursor++] === '\n' ? (endings[newline++]?.length ?? 1) : 1;
    }
    if (source > boundary.position) throw new RangeError('Position is inside CRLF');
    if (source !== boundary.position) throw new RangeError('Invalid source position');
    converted[boundary.index] = cursor;
  }
  return converted;
}
/** Convert a logical document range to the corresponding exact source range. */
export function documentRangeToSource(
  logical: string,
  endings: readonly LineEnding[],
  range: SourceRange,
): SourceRange {
  return {
    from: documentToSource(logical, endings, range.from),
    to: documentToSource(logical, endings, range.to),
  };
}
