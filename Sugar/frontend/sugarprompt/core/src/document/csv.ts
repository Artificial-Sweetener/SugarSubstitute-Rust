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
/** Decode CSV structure once while retaining exact-source UTF-16 ownership for each decoded code unit. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { freezeStructure, whitespace } from '../prompt/text.js';
import {
  prepareSourceOffsets,
  prepareValueMapping,
  type PreparedValueMapping,
} from './value-mapping.js';
/** CSV storage bounds include quoting; decoded units map doubled quotes to their full authored extent. */
export interface WildcardCsvCell {
  readonly range: SourceRange;
  /** Existing storage quoting remains authoritative even when the new value needs none. */
  readonly quoted: boolean;
  /** Exact whitespace after the closing quote, excluded from the decoded value. */
  readonly postQuoteText: string;
  readonly value: string;
  readonly characterRanges: readonly SourceRange[];
}
/** Malformed CSV retains an explicit invalid result; consumers must reject ambiguous decoded values. */
export interface WildcardCsvDocument {
  readonly valid: boolean;
  readonly records: readonly (readonly WildcardCsvCell[])[];
}
/** Canonical encoded cell and its existing scalar-safe decoded-to-storage coordinate authority. */
export interface PreparedWildcardCsvEncoding {
  readonly text: string;
  readonly quoted: boolean;
  /** Half-open UTF-16 extents are local to text, including its retained opening quote. */
  readonly mapping: PreparedValueMapping;
}
/** Encode one decoded cell while retaining existing quotes and post-quote formatting.
 * Required quoting protects delimiters; forced quoting protects an empty record at EOF or adjoining CR/LF endings.
 */
export function prepareWildcardCsvEncoding(
  cell: WildcardCsvCell,
  value: string,
  forceQuoted = false,
): PreparedWildcardCsvEncoding {
  const quoted = cell.quoted || forceQuoted || /[,"\r\n]/u.test(value),
    parts: string[] = [],
    ranges: SourceRange[] = [];
  let cursor = quoted ? 1 : 0;
  // eslint-disable-next-line @typescript-eslint/prefer-for-of -- CSV source mappings iterate UTF-16 units, preserving surrogate offsets.
  for (let at = 0; at < value.length; at++) {
    const literal = quoted && value[at] === '"' ? '""' : requireValue(value[at], 'csv');
    parts.push(literal);
    ranges.push({ from: cursor, to: cursor + literal.length });
    cursor += literal.length;
  }
  const content = parts.join(''),
    text = quoted ? `"${content}"${cell.postQuoteText}` : content;
  return freezeStructure({
    text,
    quoted,
    mapping: prepareValueMapping(prepareSourceOffsets(text), value, ranges, quoted ? 1 : 0),
  });
}
/** Parse quoted commas, doubled quotes, multiline cells, post-quote whitespace and exact CR/LF/CRLF records. */
export function parseWildcardCsv(source: string): WildcardCsvDocument {
  const records: WildcardCsvCell[][] = [];
  let row: WildcardCsvCell[] = [],
    index = 0,
    valid = true;
  while (index <= source.length) {
    const from = index,
      characters: string[] = [],
      characterRanges: SourceRange[] = [];
    const quoted = source[index] === '"';
    let postQuoteText = '';
    if (quoted) {
      index++;
      let closed = false;
      while (index < source.length) {
        if (source[index] !== '"') {
          characters.push(requireValue(source[index], 'csv'));
          characterRanges.push({ from: index, to: index + 1 });
          index++;
        } else if (source[index + 1] === '"') {
          characters.push('"');
          characterRanges.push({ from: index, to: index + 2 });
          index += 2;
        } else {
          index++;
          closed = true;
          break;
        }
      }
      if (!closed) valid = false;
      const postQuoteStart = index;
      while (index < source.length && !/[,\r\n]/u.test(requireValue(source[index], 'csv'))) {
        if (!whitespace.test(requireValue(source[index], 'csv'))) valid = false;
        index++;
      }
      postQuoteText = source.slice(postQuoteStart, index);
    } else {
      while (index < source.length && !/[,\r\n]/u.test(requireValue(source[index], 'csv'))) {
        if (source[index] === '"') valid = false;
        characters.push(requireValue(source[index], 'csv'));
        characterRanges.push({ from: index, to: index + 1 });
        index++;
      }
    }
    row.push({
      range: { from, to: index },
      quoted,
      postQuoteText,
      value: characters.join(''),
      characterRanges,
    });
    if (index >= source.length) {
      records.push(row);
      break;
    }
    if (source[index] === ',') {
      index++;
      continue;
    }
    if (source[index++] === '\r' && source[index] === '\n') index++;
    records.push(row);
    row = [];
    if (index === source.length) break;
  }
  return freezeStructure({ valid, records });
}
