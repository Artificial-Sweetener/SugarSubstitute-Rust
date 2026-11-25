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
/**
 * The shared search highlights, as the pack's own modules reach them. The prompt editor ranges its
 * matches through the same SugarPrompt module, which the web build bundles into this one file.
 */
export {
  SEARCH_CURRENT_HIGHLIGHT,
  SEARCH_MATCH_HIGHLIGHT,
  SearchHighlightRanges,
} from '../../../../sugarprompt/services/src/search/highlights.js';
