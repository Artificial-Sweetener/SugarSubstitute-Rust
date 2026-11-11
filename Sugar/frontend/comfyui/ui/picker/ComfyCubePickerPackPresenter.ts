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
/** Present Cube pack provenance without owning picker lifecycle or identity matching. */

import type { ComfyCubeNodeDefinition } from './ComfyCubeNodeDefProjector.js';

/** Mark the native category leaf already claimed for one projected Cube definition. */
export const PRESENTED_PACK_TYPE_ATTRIBUTE = 'data-sugarcubes-picker-result-type';

/** Replace only Comfy's structural category detail with resolved Cube pack identity. */
export class ComfyCubePickerPackPresenter {
  /** Present one already-identified Cube result and fail closed on host mismatch. */
  present(result: HTMLElement, definition: ComfyCubeNodeDefinition): void {
    const expectedCategory = definition.category.replaceAll('/', ' / ');
    const markedCategory = result.querySelector<HTMLElement>(`[${PRESENTED_PACK_TYPE_ATTRIBUTE}]`);
    const category =
      markedCategory ??
      [...result.querySelectorAll<HTMLElement>('span')].find(
        (span) => span.textContent?.trim() === expectedCategory,
      );
    if (!category) return;
    category.setAttribute(PRESENTED_PACK_TYPE_ATTRIBUTE, definition.name);
    if (category.textContent !== definition.sugarcubes_pack_name) {
      category.textContent = definition.sugarcubes_pack_name;
    }
  }
}
