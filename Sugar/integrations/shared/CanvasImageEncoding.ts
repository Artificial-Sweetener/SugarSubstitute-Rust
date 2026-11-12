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
/** Encode native image transfers using the common bounded byte transport. */
import type { CanvasEncodedImage } from '../contracts/CanvasProtocol.generated.js';
import { canvasPngDimensions, MAX_CANVAS_IMAGE_BYTES } from './CanvasImagePolicy.js';
import { encodeCanvasBytes, decodeCanvasBytes } from './CanvasByteEncoding.js';
/** Bound retained wire data to one admitted image. */
export const MAX_CANVAS_ENCODED_CHARACTERS = Math.ceil(MAX_CANVAS_IMAGE_BYTES / 3) * 4;
/** Validate PNG geometry before encoding full-resolution bytes. */
export function encodeCanvasImage(bytes: Uint8Array): CanvasEncodedImage {
  canvasPngDimensions(bytes);
  return { format: 'png', encoding: 'base64', data: encodeCanvasBytes(bytes) };
}
/** Validate bounded bytes and PNG geometry before native image creation. */
export function decodeCanvasImage(image: CanvasEncodedImage): Uint8Array {
  const bytes = decodeCanvasBytes(image.data, MAX_CANVAS_IMAGE_BYTES);
  canvasPngDimensions(bytes);
  return bytes;
}
