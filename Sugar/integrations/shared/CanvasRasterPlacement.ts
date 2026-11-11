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
/** Place native cropped rasters into their full document frame without stretching the occupied region. */
import type { CanvasPixelRaster } from '../contracts/CanvasProtocol.generated.js';
import { canvasRasterByteLength, encodeCanvasRaster } from './CanvasRasterEncoding.js';

/** Express the actual full-resolution document rectangle represented by returned native pixels. */
export interface CanvasRasterBounds {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

/** Reconstruct full-canvas coordinates from a bounded host crop, preserving straight color channels. */
export function placeCanvasRaster(
  document: { readonly width: number; readonly height: number },
  source: {
    readonly width: number;
    readonly height: number;
    readonly bounds: CanvasRasterBounds;
    readonly bytes: Uint8Array;
  },
  format: CanvasPixelRaster['format'],
  maximumSize: number,
): CanvasPixelRaster {
  if (
    ![document.width, document.height].every(
      (value) => Number.isSafeInteger(value) && value >= 1 && value <= 65536,
    )
  )
    throw new Error('Invalid canvas document geometry.');
  if (!Number.isSafeInteger(maximumSize) || maximumSize < 0 || maximumSize > 1024)
    throw new Error('Invalid canvas thumbnail size.');
  const scale = maximumSize
    ? Math.min(1, maximumSize / Math.max(document.width, document.height))
    : 1;
  const width = Math.max(1, Math.round(document.width * scale)),
    height = Math.max(1, Math.round(document.height * scale));
  const output = new Uint8Array(canvasRasterByteLength(width, height, format));
  if (source.width === 0 && source.height === 0 && source.bytes.length === 0)
    return encodeCanvasRaster(width, height, format, output);
  if (source.bytes.length !== canvasRasterByteLength(source.width, source.height, format))
    throw new Error('The cropped canvas raster has an invalid byte count.');
  const { left, top, right, bottom } = source.bounds;
  if (![left, top, right, bottom].every(Number.isFinite) || right <= left || bottom <= top)
    throw new Error('The cropped canvas raster has invalid document bounds.');
  const scaleX = width / document.width,
    scaleY = height / document.height;
  const x0 = Math.round(left * scaleX),
    y0 = Math.round(top * scaleY),
    x1 = Math.max(x0 + 1, Math.round(right * scaleX)),
    y1 = Math.max(y0 + 1, Math.round(bottom * scaleY));
  const components = format === 'rgba8' ? 4 : 1;
  for (let y = Math.max(0, y0); y < Math.min(height, y1); y++) {
    const sy = Math.max(
      0,
      Math.min(source.height - 1, ((y - y0 + 0.5) / (y1 - y0)) * source.height - 0.5),
    );
    const ya = Math.floor(sy),
      yb = Math.min(source.height - 1, ya + 1),
      fy = sy - ya;
    for (let x = Math.max(0, x0); x < Math.min(width, x1); x++) {
      const sx = Math.max(
        0,
        Math.min(source.width - 1, ((x - x0 + 0.5) / (x1 - x0)) * source.width - 0.5),
      );
      const xa = Math.floor(sx),
        xb = Math.min(source.width - 1, xa + 1),
        fx = sx - xa;
      for (let component = 0; component < components; component++) {
        const upper =
          (source.bytes[(ya * source.width + xa) * components + component] ?? 0) * (1 - fx) +
          (source.bytes[(ya * source.width + xb) * components + component] ?? 0) * fx;
        const lower =
          (source.bytes[(yb * source.width + xa) * components + component] ?? 0) * (1 - fx) +
          (source.bytes[(yb * source.width + xb) * components + component] ?? 0) * fx;
        output[(y * width + x) * components + component] = Math.round(
          upper * (1 - fy) + lower * fy,
        );
      }
    }
  }
  return encodeCanvasRaster(width, height, format, output);
}
