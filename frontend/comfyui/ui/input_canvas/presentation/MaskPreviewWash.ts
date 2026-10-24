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
/** Composite effective mask coverage once over the unchanged full-color document preview. */
import { decodeCanvasRaster, type CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { INPUT_PREVIEW_MAXIMUM } from '../domain/InputPreviewResolution.js';

/** Apply one half-opacity neutral wash, retaining soft coverage and straight-alpha composition pixels. */
export function maskPreviewPixels(
  mask: CanvasPixelRaster,
  composition: CanvasPixelRaster,
): Uint8ClampedArray {
  if (mask.format !== 'coverage8' || composition.format !== 'rgba8')
    throw new Error(
      'A mask preview requires an effective monochrome mask and a color composition.',
    );
  if (composition.width > INPUT_PREVIEW_MAXIMUM || composition.height > INPUT_PREVIEW_MAXIMUM)
    throw new Error('The composition preview exceeds its display limit.');
  const coverage = decodeCanvasRaster(mask),
    image = decodeCanvasRaster(composition),
    result = new Uint8ClampedArray(image.length);
  for (let y = 0; y < composition.height; y++)
    for (let x = 0; x < composition.width; x++) {
      const value = sampleCoverage(
        coverage,
        mask.width,
        mask.height,
        ((x + 0.5) * mask.width) / composition.width - 0.5,
        ((y + 0.5) * mask.height) / composition.height - 0.5,
      );
      const offset = (y * composition.width + x) * 4,
        retainedAlpha = ((image[offset + 3] ?? 0) / 255) * 0.5,
        alpha = 0.5 + retainedAlpha;
      for (let channel = 0; channel < 3; channel++)
        result[offset + channel] = Math.round(
          (value * 0.5 + (image[offset + channel] ?? 0) * retainedAlpha) / alpha,
        );
      result[offset + 3] = Math.round(alpha * 255);
    }
  return result;
}

/** Match pixel-center bilinear scaling when native loader output geometry differs from the composition. */
function sampleCoverage(
  bytes: Uint8Array,
  width: number,
  height: number,
  x: number,
  y: number,
): number {
  x = Math.max(0, Math.min(width - 1, x));
  y = Math.max(0, Math.min(height - 1, y));
  const left = Math.floor(x),
    top = Math.floor(y),
    right = Math.min(left + 1, width - 1),
    bottom = Math.min(top + 1, height - 1),
    horizontal = x - left,
    vertical = y - top;
  const upper =
      (bytes[top * width + left] ?? 0) * (1 - horizontal) +
      (bytes[top * width + right] ?? 0) * horizontal,
    lower =
      (bytes[bottom * width + left] ?? 0) * (1 - horizontal) +
      (bytes[bottom * width + right] ?? 0) * horizontal;
  return upper * (1 - vertical) + lower * vertical;
}
