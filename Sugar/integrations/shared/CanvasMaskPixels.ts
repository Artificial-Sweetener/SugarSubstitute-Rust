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
/** Bound and encode lossless grayscale coverage independently of browser or editor APIs. */
import type { CanvasMaskPixels } from '../contracts/CanvasMasksProtocol.generated.js';
import { encodeCanvasBytes, decodeCanvasBytes } from './CanvasByteEncoding.js';
/** Bound both raw coverage allocation and encoded transport size. */
export const MAX_MASK_PIXELS = 64 * 1024 * 1024;
/** Encode exactly one byte per pixel without lossy image compression. */
export function encodeMaskPixels(
  width: number,
  height: number,
  bytes: Uint8Array,
): CanvasMaskPixels {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 65536 ||
    height > 65536 ||
    width * height !== bytes.length ||
    bytes.length > MAX_MASK_PIXELS
  )
    throw new Error('Invalid or oversized mask coverage.');
  return { width, height, data: encodeCanvasBytes(bytes) };
}
/** Reject malformed bytes and dimensions before allocation or native image creation. */
export function decodeMaskPixels(pixels: CanvasMaskPixels): Uint8Array {
  const size = pixels.width * pixels.height;
  if (
    !Number.isSafeInteger(pixels.width) ||
    !Number.isSafeInteger(pixels.height) ||
    pixels.width < 1 ||
    pixels.height < 1 ||
    pixels.width > 65536 ||
    pixels.height > 65536 ||
    size > MAX_MASK_PIXELS
  )
    throw new Error('Invalid or oversized mask coverage.');
  const bytes = decodeCanvasBytes(pixels.data, MAX_MASK_PIXELS);
  if (bytes.length !== size) throw new Error('Invalid mask coverage dimensions.');
  return bytes;
}
