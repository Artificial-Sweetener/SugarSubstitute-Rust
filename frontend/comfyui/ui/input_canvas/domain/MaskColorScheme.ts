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
/** Colour input masks from the theme accent through SugarPrompt's shared ordinal palette. */
import {
  formatRegionHex,
  parseRegionHex,
  regionPaletteColor,
} from '../../../../sugarprompt/core/src/index.js';

/** Overlay opacity given to masks whose node has no stored visual opacity. */
export const DEFAULT_MASK_OPACITY = 0.5;

/**
 * Return the colour of mask `index` among `count` related masks.
 *
 * Mask 0 keeps the theme accent; later masks rotate from it through the shared ordinal palette,
 * the palette SugarPrompt colours separators with when given the same accent. Scalar masks relate
 * to every mask on their surface; the regions of an ordered loader relate to that loader's regions.
 */
export function maskColor(accent: string, index: number, count: number): string {
  const base = parseRegionHex(accent);
  if (!base) throw new Error('A mask colour requires a #rrggbb accent.');
  return formatRegionHex(regionPaletteColor(base, index, count));
}
