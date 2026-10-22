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
/** Shared scalar length and ordering keep prepared catalog comparisons consistent with Python strings. */
import { requireValue } from '../../../core/src/required.js';
/** Count string-iterator scalars without allocating; valid surrogate pairs count once and isolated surrogates count individually. */
export function scalarTextLength(value: string): number {
  let length = 0;
  for (let index = 0; index < value.length; length++)
    index += requireValue(value.codePointAt(index), 'scalar') > 0xffff ? 2 : 1;
  return length;
}
/** Compare Unicode scalar values, including astral and isolated surrogate values, without locale-specific sorting. */
export function compareScalarText(left: string, right: string): number {
  if (left === right) return 0;
  let leftIndex = 0,
    rightIndex = 0;
  while (leftIndex < left.length && rightIndex < right.length) {
    const a = requireValue(left.codePointAt(leftIndex), 'scalar'),
      b = requireValue(right.codePointAt(rightIndex), 'scalar'),
      difference = a - b;
    if (difference) return difference;
    leftIndex += a > 0xffff ? 2 : 1;
    rightIndex += b > 0xffff ? 2 : 1;
  }
  let remaining = 0;
  while (leftIndex < left.length) {
    leftIndex += requireValue(left.codePointAt(leftIndex), 'scalar') > 0xffff ? 2 : 1;
    remaining++;
  }
  while (rightIndex < right.length) {
    rightIndex += requireValue(right.codePointAt(rightIndex), 'scalar') > 0xffff ? 2 : 1;
    remaining--;
  }
  return remaining;
}
