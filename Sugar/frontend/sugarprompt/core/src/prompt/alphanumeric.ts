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
/** Pinned alphanumeric classification owns delimiter, apostrophe and prompt-word syntax decisions. */
import { requireValue } from '../required.js';
import { alphanumericIntervals } from './unicode-alphanumeric-data.js';
import { decodeUnicodeIntervals, unicodeIntervalIncludes } from './unicode-intervals.js';
const intervals = decodeUnicodeIntervals(alphanumericIntervals);
/** Match Python 3.12.10 str.isalnum: nonempty text consists entirely of Unicode 15 letters or numeric characters. */
export function unicodeAlphanumeric(text: string): boolean {
  if (!text) return false;
  for (const character of text)
    if (!unicodeIntervalIncludes(intervals, requireValue(character.codePointAt(0), 'alphanumeric')))
      return false;
  return true;
}
