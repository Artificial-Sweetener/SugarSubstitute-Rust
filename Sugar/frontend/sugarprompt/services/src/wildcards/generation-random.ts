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
/** Seeded MT19937 integer selection matches the pinned Python Random integer-seed/randrange owner. */
import { requireValue } from '../../../core/src/required.js';
/** Match Python integer-seeded random selection for the standalone wildcard service. */
export class WildcardRandom {
  private readonly state = new Uint32Array(624);
  private position = 624;
  /** Seed from absolute little-endian 32-bit words, including Python's single zero word. */
  constructor(seed: bigint) {
    let value = seed < 0n ? -seed : seed;
    const key: number[] = [];
    do {
      key.push(Number(value & 0xffffffffn));
      value >>= 32n;
    } while (value);
    this.state[0] = 19650218;
    for (let i = 1; i < 624; i++)
      this.state[i] =
        (Math.imul(
          requireValue(this.state[i - 1], 'generation-random') ^
            (requireValue(this.state[i - 1], 'generation-random') >>> 30),
          1812433253,
        ) +
          i) >>>
        0;
    let i = 1,
      j = 0;
    for (let k = Math.max(624, key.length); k; k--) {
      this.state[i] =
        ((requireValue(this.state[i], 'generation-random') ^
          Math.imul(
            requireValue(this.state[i - 1], 'generation-random') ^
              (requireValue(this.state[i - 1], 'generation-random') >>> 30),
            1664525,
          )) +
          requireValue(key[j], 'generation-random') +
          j) >>>
        0;
      if (++i >= 624) {
        this.state[0] = requireValue(this.state[623], 'generation-random');
        i = 1;
      }
      if (++j >= key.length) j = 0;
    }
    for (let k = 623; k; k--) {
      this.state[i] =
        ((requireValue(this.state[i], 'generation-random') ^
          Math.imul(
            requireValue(this.state[i - 1], 'generation-random') ^
              (requireValue(this.state[i - 1], 'generation-random') >>> 30),
            1566083941,
          )) -
          i) >>>
        0;
      if (++i >= 624) {
        this.state[0] = requireValue(this.state[623], 'generation-random');
        i = 1;
      }
    }
    this.state[0] = 0x80000000;
  }
  /** Return one tempered 32-bit word, regenerating the fixed-size state in place. */
  private word(): number {
    if (this.position >= 624) {
      for (let i = 0; i < 624; i++) {
        const y =
          (requireValue(this.state[i], 'generation-random') & 0x80000000) |
          (requireValue(this.state[(i + 1) % 624], 'generation-random') & 0x7fffffff);
        this.state[i] =
          (requireValue(this.state[(i + 397) % 624], 'generation-random') ^
            (y >>> 1) ^
            (y & 1 ? 0x9908b0df : 0)) >>>
          0;
      }
      this.position = 0;
    }
    let y = requireValue(this.state[this.position++], 'generation-random');
    y ^= y >>> 11;
    y ^= (y << 7) & 0x9d2c5680;
    y ^= (y << 15) & 0xefc60000;
    y ^= y >>> 18;
    return y >>> 0;
  }
  /** Preserve Python's rejection-sampling bit length, including the draw consumed for one-item sources. */
  index(count: number): number {
    if (!Number.isSafeInteger(count) || count < 1 || count > 250000)
      throw new RangeError('Invalid wildcard candidate count');
    const bits = Math.floor(Math.log2(count)) + 1;
    for (let draw = 0; draw < 1024; draw++) {
      const value = this.word() >>> (32 - bits);
      if (value < count) return value;
    }
    throw new Error('wildcard-random-rejection-invariant');
  }
}
