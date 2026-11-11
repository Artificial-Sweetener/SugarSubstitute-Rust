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
/** Prove PNG admission preserves straight alpha and bounds decompression before native allocation. */
import { crc32, deflateSync } from 'node:zlib';
import { photoshopPngPixels } from '../../../integrations/photoshop/src/infrastructure/PhotoshopPngPixels.js';

/** Produce PNG fixtures with valid checksums and unfiltered scanlines. */
function encode(value: {
  width: number;
  height: number;
  data: Uint8Array;
  channels: number;
}): Uint8Array {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(value.width, 0);
  header.writeUInt32BE(value.height, 4);
  header[8] = 8;
  header[9] = value.channels === 4 ? 6 : 2;
  const row = value.width * value.channels,
    raw = Buffer.alloc((row + 1) * value.height);
  for (let y = 0; y < value.height; y++)
    raw.set(value.data.subarray(y * row, (y + 1) * row), y * (row + 1) + 1);
  const chunk = (name: string, data: Buffer): Buffer => {
    const tag = Buffer.from(name),
      length = Buffer.alloc(4),
      crc = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    crc.writeUInt32BE(crc32(Buffer.concat([tag, data])));
    return Buffer.concat([length, tag, data, crc]);
  };
  return new Uint8Array(
    Buffer.concat([
      Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
      chunk('IHDR', header),
      chunk('IDAT', deflateSync(raw)),
      chunk('IEND', Buffer.alloc(0)),
    ]),
  );
}

test('decodes normalized RGB and RGBA without changing transparent or soft-alpha pixels', () => {
  const rgba = new Uint8Array([255, 0, 0, 0, 20, 40, 60, 127, 0, 120, 255, 255]);
  expect([
    ...photoshopPngPixels(encode({ width: 3, height: 1, data: rgba, channels: 4 })).pixels,
  ]).toEqual([...rgba]);
  expect([
    ...photoshopPngPixels(
      encode({ width: 1, height: 1, data: new Uint8Array([10, 20, 30]), channels: 3 }),
    ).pixels,
  ]).toEqual([10, 20, 30, 255]);
});

test('rejects corrupt PNGs and an inflated stream larger than its declared geometry', () => {
  const bytes = encode({
    width: 32,
    height: 32,
    data: new Uint8Array(32 * 32 * 4).fill(255),
    channels: 4,
  });
  const corrupt = new Uint8Array(bytes);
  corrupt[29] = corrupt[29]! ^ 1;
  expect(() => photoshopPngPixels(corrupt)).toThrow();
  const bomb = new Uint8Array(bytes);
  const view = new DataView(bomb.buffer);
  view.setUint32(16, 1);
  view.setUint32(20, 1);
  view.setUint32(29, crc32(bomb.subarray(12, 29)));
  expect(() => photoshopPngPixels(bomb)).toThrow('exceeds its declared frame');
  expect(() => photoshopPngPixels(bytes.subarray(0, bytes.length - 12))).toThrow();
});

test('rejects repeated geometry headers before third-party decoding', () => {
  const bytes = encode({ width: 1, height: 1, data: new Uint8Array(4), channels: 4 });
  const duplicate = new Uint8Array(bytes.length + 25);
  duplicate.set(bytes.subarray(0, 33));
  duplicate.set(bytes.subarray(8, 33), 33);
  duplicate.set(bytes.subarray(33), 58);
  expect(() => photoshopPngPixels(duplicate)).toThrow('one leading header');
});
