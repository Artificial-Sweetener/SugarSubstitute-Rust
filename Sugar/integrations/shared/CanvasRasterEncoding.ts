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
/** Validate portable coverage and RGBA rasters independently of their eventual consumer. */
import type { CanvasPixelRaster } from '../contracts/CanvasProtocol.generated.js';
import { decodeCanvasBytes, encodeCanvasBytes } from './CanvasByteEncoding.js';

/** Bound raw raster transfers before allocating decoded bytes. */
export const MAX_CANVAS_RASTER_BYTES = 64 * 1024 * 1024;

/** Admit exact raster geometry and pixel packing at an external adapter boundary. */
export function canvasRasterByteLength(
  width: number,
  height: number,
  format: CanvasPixelRaster['format'],
): number {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 65536 ||
    height > 65536
  )
    throw new Error('Invalid canvas raster geometry or format.');
  const size = width * height * (format === 'rgba8' ? 4 : 1);
  if (size > MAX_CANVAS_RASTER_BYTES)
    throw new Error('The canvas raster exceeds its transfer budget.');
  return size;
}

/** Encode admitted pixels without changing color channels or selection coverage. */
export function encodeCanvasRaster(
  width: number,
  height: number,
  format: CanvasPixelRaster['format'],
  bytes: Uint8Array,
): CanvasPixelRaster {
  if (bytes.length !== canvasRasterByteLength(width, height, format))
    throw new Error('The canvas raster byte count differs from its geometry.');
  return { width, height, format, data: encodeCanvasBytes(bytes) };
}

/** Decode canonical bytes only after validating the full raster budget. */
export function decodeCanvasRaster(pixels: CanvasPixelRaster): Uint8Array {
  const size = canvasRasterByteLength(pixels.width, pixels.height, pixels.format);
  const bytes = decodeCanvasBytes(pixels.data, size);
  if (bytes.length !== size)
    throw new Error('The canvas raster byte count differs from its geometry.');
  return bytes;
}
