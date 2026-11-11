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
/** Decode bounded, normalized generation PNGs before a native document transaction begins. */
import UPNG from 'upng-js';
import pako from 'pako';
import { canvasPngDimensions } from '../../../shared/CanvasImagePolicy.js';

const crcTable = Uint32Array.from({ length: 256 }, (_, value) => {
  for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  return value >>> 0;
});

/** Verify compressed chunks before asking a third-party decoder to interpret their contents. */
function chunkCrc(bytes: Uint8Array, start: number, end: number): number {
  let value = 0xffffffff;
  for (let index = start; index < end; index++)
    value = (crcTable[(value ^ (bytes[index] ?? 0)) & 255] ?? 0) ^ (value >>> 8);
  return (value ^ 0xffffffff) >>> 0;
}

/** Admit only the static sRGB byte layout produced by the browser image preparation owner. */
function validatePixelStream(bytes: Uint8Array, width: number, height: number): void {
  const channels = bytes[25] === 6 ? 4 : bytes[25] === 2 ? 3 : 0;
  if (bytes[24] !== 8 || !channels || bytes[26] !== 0 || bytes[27] !== 0 || bytes[28] !== 0)
    throw new Error('Canvas insertion requires a normalized 8-bit RGB or RGBA PNG.');
  const expected = height * (width * channels + 1);
  let expanded = 0,
    ended = false;
  const inflater = new pako.Inflate({ chunkSize: 65536 });
  inflater.onData = (chunk) => {
    expanded += chunk.byteLength;
    if (expanded > expected) throw new Error('PNG pixel data exceeds its declared frame.');
  };
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let offset = 8; offset + 12 <= bytes.length; ) {
    const length = view.getUint32(offset, false),
      start = offset + 8,
      end = start + length;
    if (end + 4 > bytes.length) throw new Error('Truncated PNG image.');
    if (chunkCrc(bytes, offset + 4, end) !== view.getUint32(end, false))
      throw new Error('PNG chunk checksum failed.');
    const kind = String.fromCharCode(...bytes.subarray(offset + 4, start));
    if (kind === 'IHDR' && (offset !== 8 || length !== 13))
      throw new Error('PNG geometry must have one leading header.');
    if (kind === 'iCCP' || kind === 'acTL')
      throw new Error('Normalize the PNG color profile and animation before canvas insertion.');
    if (kind === 'IDAT' && length && !inflater.push(bytes.subarray(start, end)))
      throw new Error('PNG pixel decompression failed.');
    if (kind === 'IEND') {
      ended = true;
      break;
    }
    offset = end + 4;
  }
  if (!ended || expanded !== expected || inflater.err)
    throw new Error('PNG pixel data does not match its declared frame.');
}

/** Return straight sRGB RGBA bytes without opening a Photoshop document or touching history. */
export function photoshopPngPixels(bytes: Uint8Array): {
  readonly width: number;
  readonly height: number;
  readonly pixels: Uint8Array;
} {
  const { width, height } = canvasPngDimensions(bytes);
  validatePixelStream(bytes, width, height);
  const decoded = UPNG.decode(new Uint8Array(bytes).buffer);
  if (
    decoded.width !== width ||
    decoded.height !== height ||
    decoded.depth !== 8 ||
    ![2, 6].includes(decoded.ctype)
  )
    throw new Error('The decoded PNG does not match its admitted RGBA frame.');
  const frame = UPNG.toRGBA8(decoded)[0];
  if (!frame || frame.byteLength !== width * height * 4)
    throw new Error('The decoded RGBA pixels do not match their frame.');
  const pixels = new Uint8Array(frame);
  return { width, height, pixels };
}
