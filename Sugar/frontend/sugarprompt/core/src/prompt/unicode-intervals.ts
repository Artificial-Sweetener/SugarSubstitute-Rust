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
/** Decode and search the generated Unicode 15 inclusive-interval schema independently of browser properties. */
import { requireValue } from '../required.js';
/** One inclusive code-point interval; property tables are sorted and disjoint. */
export type UnicodeInterval = readonly [from: number, to: number];
/** Decode a trusted generated hexadecimal table once into finite process memoization. */
export function decodeUnicodeIntervals(value: string): readonly UnicodeInterval[] {
  return value.split(/\s+/u).map((token) => {
    const [from, to] = token.split('-').map((point) => Number.parseInt(point, 16));
    return [
      requireValue(from, 'unicode-intervals'),
      requireValue(to, 'unicode-intervals'),
    ] as const;
  });
}
/** Prepared property lookup is logarithmic and retains the pinned database's unknown-scalar behavior. */
export function unicodeIntervalIncludes(
  ranges: readonly UnicodeInterval[],
  point: number,
): boolean {
  let from = 0,
    to = ranges.length;
  while (from < to) {
    const middle = (from + to) >>> 1,
      range = requireValue(ranges[middle], 'unicode-intervals');
    if (point < range[0]) to = middle;
    else if (point > range[1]) from = middle + 1;
    else return true;
  }
  return false;
}
