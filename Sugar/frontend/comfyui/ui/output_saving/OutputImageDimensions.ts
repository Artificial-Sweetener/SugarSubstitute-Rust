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
/** Bound encoded still-image dimensions before a browser allocates bitmap pixels. */
import {
  canvasPngDimensions,
  MAX_CANVAS_IMAGE_BYTES,
  MAX_CANVAS_IMAGE_PIXELS,
} from '../../../../integrations/shared/CanvasImagePolicy.js';
import type { ImageDimensions } from '../execution/domain/WorkflowOutput.js';

/** Reuse the established canvas pixel budget for browser clipboard and drag admission. */
export function outputImageTransferDimensions(value: ImageDimensions): boolean {
  return (
    Number.isSafeInteger(value.width) &&
    Number.isSafeInteger(value.height) &&
    value.width >= 1 &&
    value.height >= 1 &&
    value.width <= 30000 &&
    value.height <= 30000 &&
    value.width * value.height <= MAX_CANVAS_IMAGE_PIXELS
  );
}
/** Inspect only supported bounded headers; the browser codec validates the remaining stream. */
export function outputImageDimensions(bytes: Uint8Array, mime: string): ImageDimensions {
  if (!bytes.length || bytes.length > MAX_CANVAS_IMAGE_BYTES)
    throw new Error('Output image exceeds its byte limit.');
  if (mime === 'image/png') return canvasPngDimensions(bytes);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let width = 0,
    height = 0;
  if (mime === 'image/jpeg') {
    if (bytes[0] !== 0xff || bytes[1] !== 0xd8) throw new Error('Output JPEG header is invalid.');
    let position = 2;
    for (let segments = 0; segments < 10000 && position + 4 < bytes.length; segments++) {
      if (bytes[position++] !== 0xff) throw new Error('Output JPEG marker is invalid.');
      while (bytes[position] === 0xff) position++;
      const marker = bytes[position++];
      if (marker === undefined || marker === 0xda || marker === 0xd9) break;
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) continue;
      if (position + 2 > bytes.length) break;
      const length = view.getUint16(position, false);
      if (length < 2 || position + length > bytes.length) break;
      if (
        [0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf].includes(
          marker,
        )
      ) {
        if (length < 8) break;
        height = view.getUint16(position + 3, false);
        width = view.getUint16(position + 5, false);
        break;
      }
      position += length;
    }
  } else if (mime === 'image/webp') {
    const text = (start: number, length: number) =>
      String.fromCharCode(...bytes.subarray(start, start + length));
    if (
      bytes.length < 25 ||
      text(0, 4) !== 'RIFF' ||
      text(8, 4) !== 'WEBP' ||
      view.getUint32(4, true) + 8 !== bytes.length
    )
      throw new Error('Output WebP header is invalid.');
    const kind = text(12, 4);
    if (kind === 'VP8X' && bytes.length >= 30) {
      width = 1 + ((bytes[24] ?? 0) | ((bytes[25] ?? 0) << 8) | ((bytes[26] ?? 0) << 16));
      height = 1 + ((bytes[27] ?? 0) | ((bytes[28] ?? 0) << 8) | ((bytes[29] ?? 0) << 16));
    } else if (kind === 'VP8 ' && bytes.length >= 30 && text(23, 3) === '\x9d\x01\x2a') {
      width = view.getUint16(26, true) & 0x3fff;
      height = view.getUint16(28, true) & 0x3fff;
    } else if (kind === 'VP8L' && bytes[20] === 0x2f) {
      const bits = view.getUint32(21, true);
      width = (bits & 0x3fff) + 1;
      height = ((bits >>> 14) & 0x3fff) + 1;
    }
  }
  if (!outputImageTransferDimensions({ width, height }))
    throw new Error('Output image exceeds its decoded pixel limit or has invalid dimensions.');
  return Object.freeze({ width, height });
}
