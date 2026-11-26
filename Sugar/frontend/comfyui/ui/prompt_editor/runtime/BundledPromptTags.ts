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
/** Share one prepared tag catalog across editor and renderer lifetimes, per autocomplete lists. */
import tags from '../assets/danbooru-tags.txt';
import { bundledPromptTags, preparePromptTagCatalog } from './PromptTagCatalog.js';
import type {
  BundledTag,
  TagCompletionCatalog,
} from '../../../../sugarprompt/services/src/index.js';
import type { PromptTagLists } from '../../autocomplete_lists/application/AutocompleteListLibrary.js';

const BUNDLED_REVISION = 'sha256:38a5b8d1ff959d927d47423191575ea1281ee0c5e1a77e5773967ca1dfab5f82';

let bundled: readonly BundledTag[] | undefined;
let prepared:
  | { readonly lists: PromptTagLists | null; readonly catalog: TagCompletionCatalog }
  | undefined;

/**
 * The bundled snapshot with the enabled autocomplete lists' tags: custom tags added, censored
 * tags hidden. Every editor shares the catalog of the newest lists, prepared once per revision.
 */
export function promptTagCatalog(lists: PromptTagLists | null): TagCompletionCatalog {
  if (prepared?.lists === lists) return prepared.catalog;
  bundled ??= bundledPromptTags(tags);
  const catalog = preparePromptTagCatalog(
    bundled,
    lists ? `${BUNDLED_REVISION}:lists:${lists.revision}` : BUNDLED_REVISION,
    lists?.custom ?? [],
    lists?.censored ?? [],
  );
  prepared = { lists, catalog };
  return catalog;
}
