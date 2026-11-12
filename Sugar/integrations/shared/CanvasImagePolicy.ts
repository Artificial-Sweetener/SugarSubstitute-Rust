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
/** Bound encoded PNG admission independently of native decoders and transports. */
export const MAX_CANVAS_IMAGE_BYTES = 64 * 1024 * 1024;
/** Preserve full-image admission independently of the encoded PNG byte budget. */
export const MAX_CANVAS_IMAGE_PIXELS = 64 * 1024 * 1024;
/** Read intrinsic PNG dimensions before an editor allocates decoded pixels. */
export function canvasPngDimensions(bytes: Uint8Array): {
  readonly width: number;
  readonly height: number;
} {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (
    bytes.length < 33 ||
    bytes.length > MAX_CANVAS_IMAGE_BYTES ||
    signature.some((byte, index) => bytes[index] !== byte) ||
    bytes[12] !== 73 ||
    bytes[13] !== 72 ||
    bytes[14] !== 68 ||
    bytes[15] !== 82
  )
    throw new Error('Canvas insertion requires a valid bounded PNG image.');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const width = view.getUint32(16, false),
    height = view.getUint32(20, false);
  if (
    width < 1 ||
    height < 1 ||
    width > 30000 ||
    height > 30000 ||
    width * height > MAX_CANVAS_IMAGE_PIXELS
  )
    throw new Error('This image exceeds the canvas insertion pixel budget.');
  return Object.freeze({ width, height });
}
