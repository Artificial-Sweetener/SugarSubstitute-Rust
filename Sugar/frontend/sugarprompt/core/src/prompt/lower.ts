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
/** Pinned lowercase identity differs from full case folding and includes contextual Greek final sigma. */
import { requireValue } from '../required.js';
import { lowercaseMappings, casedIntervals, caseIgnorableIntervals } from './unicode-lower-data.js';
import { decodeUnicodeIntervals, unicodeIntervalIncludes } from './unicode-intervals.js';
const cased = decodeUnicodeIntervals(casedIntervals),
  ignorable = decodeUnicodeIntervals(caseIgnorableIntervals);
const mappings = new Map(
  lowercaseMappings.split(/\s+/u).map((token) => {
    const [source, target] = token.split(':');
    return [
      Number.parseInt(requireValue(source, 'lower'), 16),
      String.fromCodePoint(
        ...requireValue(target, 'lower')
          .split(',')
          .map((point) => Number.parseInt(point, 16)),
      ),
    ] as const;
  }),
);
/** Lower with Python 3.12's Unicode 15 identities, retaining newer/unknown scalars and isolated surrogates literally. */
export function unicodeLowerCase(text: string): string {
  const characters = Array.from(text),
    followingCased: boolean[] = [];
  let following = false;
  for (let index = characters.length - 1; index >= 0; index--) {
    const point = requireValue(requireValue(characters[index], 'lower').codePointAt(0), 'lower');
    followingCased[index] = following;
    if (!unicodeIntervalIncludes(ignorable, point))
      following = unicodeIntervalIncludes(cased, point);
  }
  let previous = false,
    result = '';
  for (let index = 0; index < characters.length; index++) {
    const character = requireValue(characters[index], 'lower'),
      point = requireValue(character.codePointAt(0), 'lower');
    result +=
      point === 0x3a3 && previous && !followingCased[index]
        ? 'ς'
        : (mappings.get(point) ?? character);
    if (!unicodeIntervalIncludes(ignorable, point))
      previous = unicodeIntervalIncludes(cased, point);
  }
  return result;
}
