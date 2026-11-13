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
/** One native configuration input supplies the host's finite, validated post lookup. */
import { Facet } from '@codemirror/state';
import type { BooruPostLookup } from '../../services/src/booru/links.js';

/** Native clipboard work consumes the current accepted lookup identity; absence preserves ordinary literal paste. */
export const booruPostLookup = Facet.define<BooruPostLookup | null, BooruPostLookup | null>({
  combine(values) {
    if (values.length > 1) throw new Error('Multiple booru lookup owners');
    return values[0] ?? null;
  },
});
