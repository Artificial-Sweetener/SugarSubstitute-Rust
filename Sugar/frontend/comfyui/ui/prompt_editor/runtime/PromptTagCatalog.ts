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
/** Adapt the committed tag dataset to SugarPrompt's immutable ranking and exact-tag owner. */
import {
  prepareTagCompletionCatalog,
  type BundledTag,
  type TagCompletionCatalog,
} from '../../../../sugarprompt/services/src/index.js';

/** Reject damaged packaged rows rather than silently publishing an incomplete autocomplete catalog. */
export function preparePromptTagCatalog(source: string, revision: string): TagCompletionCatalog {
  const bundled: BundledTag[] = [];
  for (const [index, line] of source.split(/\r?\n/u).entries()) {
    if (!line.trim()) continue;
    const row = /^([^,]+),(\d+)(?:\s|$)/u.exec(line);
    const tag = row?.[1];
    const popularity = Number(row?.[2]);
    if (!tag || !Number.isSafeInteger(popularity) || popularity < 0)
      throw new TypeError(`Invalid bundled prompt tag at line ${String(index + 1)}.`);
    bundled.push({ tag, popularity });
  }
  if (!bundled.length) throw new TypeError('The bundled prompt tag catalog is empty.');
  return prepareTagCompletionCatalog({ revision, bundled, customTags: [], censoredTags: [] });
}
