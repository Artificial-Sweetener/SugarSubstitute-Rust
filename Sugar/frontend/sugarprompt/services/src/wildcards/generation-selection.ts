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
/** One finite pass owns all coordinated base/tag choices and seeded Python-compatible streams. */
import { requireValue } from '../../../core/src/required.js';
import { WildcardRandom } from './generation-random.js';
import { wildcardDigest } from './generation-digest.js';
/** Per-pass coordination retains exact authored identity independently of source lookup normalization. */
export class WildcardSelectionContext {
  private readonly streams = new Map<string | null, WildcardRandom>();
  private readonly bases = new Map<string, number>();
  private readonly offsets = new Map<string, number>();
  /** Capture one injected entropy integer for null-seed selection; its seed is never reported as an authored seed. */
  private readonly entropySeed: string | null;
  /** Capture the host entropy integer for one pass, without making it an effective authored seed. */
  constructor(entropySeed: string | null) {
    this.entropySeed = entropySeed;
  }
  /** Match retained source/seed grouping and stable tagged offsets; finite context saturation is explicit. */
  select(
    seed: string | null,
    form: 'simple' | 'csv',
    identifier: string,
    sourceId: string,
    count: number,
    tag: string | null,
  ): number | null {
    if (!this.streams.has(seed)) {
      if (this.streams.size >= 64) return null;
      this.streams.set(
        seed,
        new WildcardRandom(BigInt(seed ?? requireValue(this.entropySeed, 'generation-selection'))),
      );
    }
    const rng = requireValue(this.streams.get(seed), 'generation-selection'),
      key = JSON.stringify([seed, form, identifier]);
    let base = this.bases.get(key);
    if (base === undefined) {
      if (this.bases.size >= 16384) return null;
      base = rng.index(count);
      this.bases.set(key, base);
    }
    if (tag === null) return base;
    const tagKey = JSON.stringify([key, tag]);
    let offset = this.offsets.get(tagKey);
    if (offset === undefined) {
      if (this.offsets.size >= 16384) return null;
      offset =
        seed === null
          ? rng.index(count)
          : Number(wildcardDigest(`${seed}:${sourceId}:${tag}`) % BigInt(count));
      this.offsets.set(tagKey, offset);
    }
    return (base + offset) % count;
  }
  /** Retire every random stream and coordinated choice at the generation pass boundary. */
  dispose(): void {
    this.streams.clear();
    this.bases.clear();
    this.offsets.clear();
  }
}
