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
/** What prompt editors follow of the person's autocomplete lists. */
import type { PromptTagLists } from '../autocomplete_lists/application/AutocompleteListLibrary.js';

/** The tags the enabled lists add and hide now, and their changes. */
export interface PromptTagListSource {
  current(): PromptTagLists | null;
  subscribe(listener: () => void): () => void;
}

/** No lists, for an editor whose host keeps none. */
export const noTagLists: PromptTagListSource = Object.freeze({
  current: () => null,
  subscribe: () => () => undefined,
});
