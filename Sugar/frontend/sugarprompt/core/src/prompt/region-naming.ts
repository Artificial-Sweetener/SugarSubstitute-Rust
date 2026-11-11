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
/** Exact regional name edits consume the authoritative parsed separator identity. */
import type { PromptDocument } from './contracts.js';
import type { SourceReplacement } from '../normalization/contracts.js';

/** Native single-line drafts retain literal Unicode and spacing while excluding separator terminators. */
export function validRegionName(name: string): boolean {
  return !/[\]\r\n]/u.test(name);
}

/** Prepare the smallest exact-source UTF-16 edit; empty names restore the plain separator without changing partitions. */
export function regionNameReplacement(
  document: PromptDocument,
  ordinal: number,
  name: string,
): SourceReplacement {
  if (typeof name !== 'string' || !validRegionName(name))
    throw new TypeError('Invalid region name');
  if (!Number.isSafeInteger(ordinal) || ordinal < 0) throw new RangeError('Invalid region ordinal');
  const separator = document.regions.separators[ordinal];
  if (!separator) throw new RangeError('Region unavailable');
  const range = name && separator.nameRange ? separator.nameRange : separator.tokenRange;
  return Object.freeze({
    ...range,
    text: name ? (separator.nameRange ? name : `[SEP|${name}]`) : '[SEP]',
  });
}
