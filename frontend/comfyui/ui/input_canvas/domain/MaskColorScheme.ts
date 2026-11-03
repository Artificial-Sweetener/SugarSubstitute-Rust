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
/** Choose which base the shared ordinal palette rotates from for each kind of input mask. */
import {
  REGION_PALETTE_BASE,
  formatRegionHex,
  parseRegionHex,
  regionPaletteColor,
} from '../../../../sugarprompt/core/src/index.js';

/** Overlay opacity given to masks whose node has no stored visual opacity. */
export const DEFAULT_MASK_OPACITY = 0.5;

/**
 * Return the overlay colour of mask `index` among `count` masks sharing one surface.
 *
 * Mask 0 keeps the theme accent; later masks rotate from it through the shared ordinal palette.
 */
export function maskColor(accent: string, index: number, count: number): string {
  const base = parseRegionHex(accent);
  if (!base) throw new Error('A mask colour requires a #rrggbb accent.');
  return formatRegionHex(regionPaletteColor(base, index, count));
}

/**
 * Return the colour of region `index` among the `count` regions of one ordered mask loader.
 *
 * Regions take SugarPrompt's regional palette so a region's mask, its card row and its prompt
 * separator show the same colour.
 */
export function regionMaskColor(index: number, count: number): string {
  return formatRegionHex(regionPaletteColor(REGION_PALETTE_BASE, index, count));
}
