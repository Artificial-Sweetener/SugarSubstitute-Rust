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
/** Share bounded canonical base64 mechanics between image and grayscale canvas transfers. */
const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
/** Encode admitted bytes without host globals or unbounded argument spreads. */
export function encodeCanvasBytes(bytes: Uint8Array): string {
  const blocks: string[] = [];
  let block = '';
  for (let index = 0; index < bytes.length; index += 3) {
    const first = bytes[index] ?? 0,
      second = bytes[index + 1] ?? 0,
      third = bytes[index + 2] ?? 0;
    block +=
      ALPHABET.charAt(first >> 2) +
      ALPHABET.charAt(((first & 3) << 4) | (second >> 4)) +
      (index + 1 < bytes.length ? ALPHABET.charAt(((second & 15) << 2) | (third >> 6)) : '=') +
      (index + 2 < bytes.length ? ALPHABET.charAt(third & 63) : '=');
    if (block.length >= 16384) {
      blocks.push(block);
      block = '';
    }
  }
  blocks.push(block);

  return blocks.join('');
}
/** Reject noncanonical or oversized data before exposing bytes to either runtime. */
export function decodeCanvasBytes(data: string, maximumBytes: number): Uint8Array {
  if (!data.length || data.length > Math.ceil(maximumBytes / 3) * 4 || data.length % 4 !== 0)
    throw new Error('Invalid canvas byte encoding.');
  const padding = data.endsWith('==') ? 2 : data.endsWith('=') ? 1 : 0;
  const size = (data.length / 4) * 3 - padding;
  if (size > maximumBytes) throw new Error('Canvas image transfer exceeds its byte budget.');
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (let index = 0; index < data.length; index += 4) {
    const a = ALPHABET.indexOf(data.charAt(index)),
      b = ALPHABET.indexOf(data.charAt(index + 1));
    const c = ALPHABET.indexOf(data.charAt(index + 2)),
      d = ALPHABET.indexOf(data.charAt(index + 3));
    const final = index + 4 === data.length;
    if (
      a < 0 ||
      b < 0 ||
      (!final && (c < 0 || d < 0)) ||
      (final &&
        (padding === 0
          ? c < 0 || d < 0
          : padding === 1
            ? c < 0 || d !== -1 || (c & 3) !== 0
            : c !== -1 || d !== -1 || (b & 15) !== 0))
    )
      throw new Error('Invalid canvas byte encoding.');
    bytes[offset++] = (a << 2) | (b >> 4);
    if (offset < size) bytes[offset++] = ((b & 15) << 4) | (c >> 2);
    if (offset < size) bytes[offset++] = ((c & 3) << 6) | d;
  }

  return bytes;
}
