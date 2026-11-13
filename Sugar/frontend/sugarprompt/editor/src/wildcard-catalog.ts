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
/** One prepared host catalog facet governs derived wildcard rendering and completion context. */
import { Facet } from '@codemirror/state';
import type { WildcardCatalog } from '../../services/src/index.js';

/** Null means unavailable; metadata acquisition and revision preparation belong to the host/service boundary. */
export const wildcardCatalog = Facet.define<WildcardCatalog | null, WildcardCatalog | null>({
  /** Select the single explicit mount/reconfiguration authority. */
  combine(values) {
    return values[0] ?? null;
  },
});
