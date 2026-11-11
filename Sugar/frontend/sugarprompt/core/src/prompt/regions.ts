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
/** Exact whole-line regional markers and lossless partition ownership. */
import { requireValue } from '../required.js';
import type { RegionSeparator, RegionStructure, RegionToken } from './contracts.js';
import type { SourceRange } from '../offsets.js';
/** Recognize a marker at an exact-source UTF-16 offset, without crossing CR/LF lines. */
export function regionSeparatorAt(source: string, from: number): RegionSeparator | null {
  const token = regionTokenAt(source, from);
  if (
    (from > 0 && !/[\r\n]/u.test(requireValue(source[from - 1], 'regions'))) ||
    !token ||
    (token.tokenRange.to < source.length &&
      !/[\r\n]/u.test(requireValue(source[token.tokenRange.to], 'regions')))
  )
    return null;
  const to = token.tokenRange.to;
  return {
    ...token,
    lineRange: {
      from,
      to: to + (source.startsWith('\r\n', to) ? 2 : to < source.length ? 1 : 0),
    },
  };
}
/** Recognize exact inline syntax without crossing CR/LF; placement and escaping are separate authoring rules. */
export function regionTokenAt(source: string, from: number): RegionToken | null {
  if (from < 0 || !source.startsWith('[SEP', from)) return null;
  let end = from + 4,
    nameRange: SourceRange | null = null,
    name: string | null = null;
  if (source[end] === '|') {
    const nameFrom = ++end;
    while (end < source.length && !/[\]\r\n]/u.test(requireValue(source[end], 'regions'))) end++;
    if (end === nameFrom || source[end] !== ']') return null;
    nameRange = { from: nameFrom, to: end };
    name = source.slice(nameFrom, end);
  } else if (source[end] !== ']') return null;
  const to = end + 1;
  return {
    tokenRange: { from, to },
    nameRange,
    name,
  };
}
/** Locate complete tokens fully contained in an exact-source UTF-16 range. */
export function regionTokensInRange(
  source: string,
  from: number,
  to: number,
): readonly RegionToken[] {
  const tokens: RegionToken[] = [];
  let cursor = Math.max(0, from);
  while (cursor < Math.min(source.length, to)) {
    const start = source.indexOf('[SEP', cursor);
    if (start < 0 || start >= to) break;
    const token = regionTokenAt(source, start);
    if (token && token.tokenRange.to <= to) {
      tokens.push(token);
      cursor = token.tokenRange.to;
    } else cursor = start + 4;
  }
  return tokens;
}
/** Build every partition, preserving leading, adjacent and trailing empty regions. */
export function regionStructure(
  sourceLength: number,
  separators: readonly RegionSeparator[],
): RegionStructure {
  let from = 0;
  const partitions = separators.map((separator, index) => {
    const partition = {
      index,
      sourceRange: { from, to: separator.lineRange.from },
      isGlobal: index === 0,
    };
    from = separator.lineRange.to;
    return partition;
  });
  partitions.push({
    index: partitions.length,
    sourceRange: { from, to: sourceLength },
    isGlobal: partitions.length === 0,
  });
  return { separators, partitions };
}
