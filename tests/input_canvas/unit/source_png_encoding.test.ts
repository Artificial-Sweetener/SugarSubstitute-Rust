/** @jest-environment node */
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
/** Prove uploaded PNG bytes retain channels beneath full and partial transparency. */
import { inflateSync } from 'node:zlib';
import { encodeSourcePng } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/BrowserSourcePng.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

test('encodes exact straight RGBA scanlines rather than premultiplied browser canvas samples', async () => {
  const pixels = Uint8Array.from([
    201, 102, 53, 0, 11, 22, 33, 127, 77, 88, 99, 255, 19, 29, 39, 64,
  ]);
  const blob = await encodeSourcePng(encodeCanvasRaster(2, 2, 'rgba8', pixels));
  const bytes = new Uint8Array(await blob.arrayBuffer());
  expect([...bytes.slice(0, 8)]).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
  const view = new DataView(bytes.buffer),
    compressed: Uint8Array[] = [];
  let offset = 8;
  while (offset < bytes.length) {
    const size = view.getUint32(offset),
      kind = String.fromCharCode(...bytes.slice(offset + 4, offset + 8));
    if (kind === 'IHDR')
      expect([
        view.getUint32(offset + 8),
        view.getUint32(offset + 12),
        bytes[offset + 16],
        bytes[offset + 17],
      ]).toEqual([2, 2, 8, 6]);
    if (kind === 'IDAT') compressed.push(bytes.slice(offset + 8, offset + 8 + size));
    offset += size + 12;
  }
  expect([...inflateSync(Buffer.concat(compressed))]).toEqual([
    0,
    ...pixels.slice(0, 8),
    0,
    ...pixels.slice(8),
  ]);
  await expect(
    encodeSourcePng(encodeCanvasRaster(1, 1, 'coverage8', new Uint8Array(1))),
  ).rejects.toThrow('straight RGBA');
});
