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
/** Shared canonical regional names consume pure ordinal semantics while hosts retain prompt/mask authority. */
import {
  firstRegionNames,
  validRegionName,
  type PromptDocument,
  type RegionName,
} from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';

/** Capture literal host names once; duplicate ordinals and structural characters reject before source adaptation. */
export function captureRegionNames(value: unknown): readonly RegionName[] {
  if (!Array.isArray(value)) throw new TypeError('Invalid shared region names');
  const seen = new Set<number>();
  return Object.freeze(
    value.map((item: unknown) => {
      if (
        !item ||
        typeof item !== 'object' ||
        Object.keys(item).sort().join() !== 'name,ordinal' ||
        !('ordinal' in item) ||
        typeof item.ordinal !== 'number' ||
        !Number.isSafeInteger(item.ordinal) ||
        item.ordinal < 0 ||
        seen.has(item.ordinal) ||
        !('name' in item) ||
        typeof item.name !== 'string' ||
        !validRegionName(item.name)
      )
        throw new TypeError('Invalid shared region identity');
      seen.add(item.ordinal);
      return Object.freeze({ ordinal: item.ordinal, name: strip(item.name) });
    }),
  );
}

/** Stable host endpoint order chooses first-authored names; explicit changes, including clearing, take precedence. */
export function resolveSharedRegionNames(
  documents: readonly PromptDocument[],
  changes: readonly RegionName[] = [],
): readonly RegionName[] {
  const names = new Map(firstRegionNames(documents).map((value) => [value.ordinal, value]));
  for (const value of captureRegionNames(changes)) names.set(value.ordinal, value);
  return Object.freeze(Array.from(names.values()));
}
