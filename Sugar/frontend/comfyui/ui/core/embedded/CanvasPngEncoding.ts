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
/** Encode straight source channels as PNG without browser canvas premultiplication changing hidden RGB. */
import type { CanvasPixelRaster } from './CanvasRuntime.js';
import { decodeCanvasRaster } from './CanvasRuntime.js';

/** Preserve every RGBA sample, including RGB beneath transparent pixels, through normal Comfy uploads. */
export async function encodeSourcePng(pixels: CanvasPixelRaster): Promise<Blob> {
  if (pixels.format !== 'rgba8') throw new Error('PNG input must contain straight RGBA pixels.');
  const bytes = decodeCanvasRaster(pixels),
    stride = pixels.width * 4;
  const scanlines = new Uint8Array(bytes.length + pixels.height);
  for (let row = 0; row < pixels.height; row++)
    scanlines.set(bytes.subarray(row * stride, (row + 1) * stride), row * (stride + 1) + 1);
  const compressed = await new Response(
    new Blob([scanlines]).stream().pipeThrough(new CompressionStream('deflate')),
  ).arrayBuffer();
  const header = new Uint8Array(13),
    view = new DataView(header.buffer);
  view.setUint32(0, pixels.width);
  view.setUint32(4, pixels.height);
  header[8] = 8;
  header[9] = 6;
  return new Blob(
    [
      Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10]),
      chunk('IHDR', header),
      chunk('IDAT', new Uint8Array(compressed)),
      chunk('IEND', new Uint8Array()),
    ],
    { type: 'image/png' },
  );
}

/** Frame PNG chunks with their network-order length and ISO 3309 CRC. */
function chunk(kind: string, data: Uint8Array): Uint8Array<ArrayBuffer> {
  const result = new Uint8Array(data.length + 12),
    view = new DataView(result.buffer);
  view.setUint32(0, data.length);
  for (let index = 0; index < 4; index++) result[index + 4] = kind.charCodeAt(index);
  result.set(data, 8);
  let crc = 0xffffffff;
  for (const byte of result.subarray(4, result.length - 4)) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  view.setUint32(result.length - 4, (crc ^ 0xffffffff) >>> 0);
  return result;
}
