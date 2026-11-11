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
/** Combine canvas image content and selection coverage without changing the document coordinate frame. */
import type { CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { decodeCanvasRaster, encodeCanvasRaster } from '../../core/embedded/CanvasRuntime.js';
/** Retain straight image colors under selected coverage and remove unselected content from RGB-only consumers. */
export function canvasSelectionImage(
  image: CanvasPixelRaster,
  selection: CanvasPixelRaster,
): CanvasPixelRaster {
  if (
    image.format !== 'rgba8' ||
    selection.format !== 'coverage8' ||
    image.width !== selection.width ||
    image.height !== selection.height
  )
    throw new Error('Selection image content and coverage must share one full-canvas frame.');
  const rgba = decodeCanvasRaster(image),
    coverage = decodeCanvasRaster(selection),
    result = new Uint8Array(rgba.length);
  for (let index = 0; index < coverage.length; index++) {
    const offset = index * 4,
      alpha = Math.round(((rgba[offset + 3] ?? 0) * (coverage[index] ?? 0)) / 255);
    // Comfy IMAGE outputs discard alpha; keep content outside the selected shape out of those outputs too.
    if (alpha) {
      result[offset] = rgba[offset] ?? 0;
      result[offset + 1] = rgba[offset + 1] ?? 0;
      result[offset + 2] = rgba[offset + 2] ?? 0;
    }
    result[offset + 3] = alpha;
  }
  return encodeCanvasRaster(image.width, image.height, 'rgba8', result);
}
