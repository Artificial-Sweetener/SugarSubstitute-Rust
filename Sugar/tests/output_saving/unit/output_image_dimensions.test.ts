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
/** Check encoded geometry admission before browser image allocation. */
import { outputImageDimensions } from '../../../frontend/comfyui/ui/output_saving/OutputImageDimensions.js';

/** Make only the bounded PNG header needed by this admission owner. */
function png(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(33);
  bytes.set([137, 80, 78, 71, 13, 10, 26, 10]);
  bytes.set([73, 72, 68, 82], 12);
  const view = new DataView(bytes.buffer);
  view.setUint32(16, width);
  view.setUint32(20, height);
  return bytes;
}

/** Make a bounded VP8X, VP8L or VP8 header independently of the browser decoder. */
function webp(kind: 'VP8X' | 'VP8L' | 'VP8 '): Uint8Array {
  const bytes = new Uint8Array(kind === 'VP8L' ? 25 : 30);
  const text = new TextEncoder();
  bytes.set(text.encode('RIFF'));
  bytes.set(text.encode('WEBP'), 8);
  bytes.set(text.encode(kind), 12);
  const view = new DataView(bytes.buffer);
  view.setUint32(4, bytes.length - 8, true);
  view.setUint32(16, bytes.length - 20, true);
  if (kind === 'VP8X') {
    bytes[24] = 3;
    bytes[27] = 2;
  } else if (kind === 'VP8L') {
    bytes[20] = 0x2f;
    view.setUint32(21, 3 | (2 << 14), true);
  } else {
    bytes.set([0x9d, 0x01, 0x2a], 23);
    view.setUint16(26, 4, true);
    view.setUint16(28, 3, true);
  }
  return bytes;
}

test('reads supported PNG and JPEG intrinsic dimensions without invoking browser decode', () => {
  expect(outputImageDimensions(png(4, 3), 'image/png')).toEqual({ width: 4, height: 3 });
  const jpeg = new Uint8Array([
    0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 3, 0, 4, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0, 0xff, 0xd9,
  ]);
  expect(outputImageDimensions(jpeg, 'image/jpeg')).toEqual({ width: 4, height: 3 });
});

test.each(['VP8X', 'VP8L', 'VP8 '] as const)(
  'reads the %s WebP dimension representation',
  (kind) => {
    expect(outputImageDimensions(webp(kind), 'image/webp')).toEqual({ width: 4, height: 3 });
  },
);

test('rejects decompression-bomb geometry, incomplete segment lengths and wrong media headers', () => {
  expect(() => outputImageDimensions(png(30000, 30000), 'image/png')).toThrow('pixel budget');
  expect(() =>
    outputImageDimensions(
      new Uint8Array([0xff, 0xd8, 0xff, 0xc0, 0xff, 0xff, 8, 0, 1]),
      'image/jpeg',
    ),
  ).toThrow();
  const invalid = webp('VP8X');
  new DataView(invalid.buffer).setUint32(4, 1, true);
  expect(() => outputImageDimensions(invalid, 'image/webp')).toThrow('invalid');
  expect(() => outputImageDimensions(png(4, 3), 'image/jpeg')).toThrow('invalid');
});
