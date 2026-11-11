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
/** Portable SHA-256 tag offsets consume validated Unicode text outside editor hot paths. */
import { requireValue } from '../../../core/src/required.js';
const constants = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
/** Rotate a fixed 32-bit word without floating-point multiplication. */
function rotate(value: number, bits: number): number {
  return (value >>> bits) | (value << (32 - bits));
}
/** Require complete Unicode scalar sequences before Python-compatible UTF-8 hashing. */
export function isWildcardUnicode(value: string): boolean {
  return !/[\uD800-\uDFFF]/u.test(value);
}
/** Return the complete digest as an integer for exact modulo selection; unpaired surrogates reject explicitly. */
export function wildcardDigest(text: string): bigint {
  if (!isWildcardUnicode(text)) throw new TypeError('Invalid wildcard digest text');
  const input = new TextEncoder().encode(text),
    length = Math.ceil((input.length + 9) / 64) * 64;
  const bytes = new Uint8Array(length);
  bytes.set(input);
  bytes[input.length] = 0x80;
  const view = new DataView(bytes.buffer);
  view.setBigUint64(length - 8, BigInt(input.length) * 8n);
  const h = [
      0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
      0x5be0cd19,
    ],
    words = new Uint32Array(64);
  for (let offset = 0; offset < length; offset += 64) {
    for (let i = 0; i < 16; i++) words[i] = view.getUint32(offset + i * 4);
    for (let i = 16; i < 64; i++) {
      const x = requireValue(words[i - 15], 'generation-digest'),
        y = requireValue(words[i - 2], 'generation-digest');
      words[i] =
        (requireValue(words[i - 16], 'generation-digest') +
          (rotate(x, 7) ^ rotate(x, 18) ^ (x >>> 3)) +
          requireValue(words[i - 7], 'generation-digest') +
          (rotate(y, 17) ^ rotate(y, 19) ^ (y >>> 10))) >>>
        0;
    }
    let [a, b, c, d, e, f, g, z] = h as [
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
    ];
    for (let i = 0; i < 64; i++) {
      const t1 =
        (z +
          (rotate(e, 6) ^ rotate(e, 11) ^ rotate(e, 25)) +
          ((e & f) ^ (~e & g)) +
          requireValue(constants[i], 'generation-digest') +
          requireValue(words[i], 'generation-digest')) >>>
        0;
      const t2 =
        ((rotate(a, 2) ^ rotate(a, 13) ^ rotate(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
      z = g;
      g = f;
      f = e;
      e = (d + t1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) >>> 0;
    }
    const working = [a, b, c, d, e, f, g, z];
    for (let i = 0; i < 8; i++)
      h[i] =
        (requireValue(h[i], 'generation-digest') +
          requireValue(working[i], 'generation-digest')) >>>
        0;
  }
  return BigInt('0x' + h.map((word) => word.toString(16).padStart(8, '0')).join(''));
}
