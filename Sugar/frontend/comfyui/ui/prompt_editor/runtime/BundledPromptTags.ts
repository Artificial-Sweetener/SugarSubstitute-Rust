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
/** Share one prepared built-in tag catalog across editor and renderer lifetimes. */
import tags from '../assets/danbooru-tags.txt';
import { preparePromptTagCatalog } from './PromptTagCatalog.js';
import type { TagCompletionCatalog } from '../../../../sugarprompt/services/src/index.js';

let catalog: TagCompletionCatalog | undefined;

/** Prepare the bundled snapshot once, before a native editor accepts input. */
export function bundledPromptTagCatalog(): TagCompletionCatalog {
  return (catalog ??= preparePromptTagCatalog(
    tags,
    'sha256:38a5b8d1ff959d927d47423191575ea1281ee0c5e1a77e5773967ca1dfab5f82',
  ));
}
