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
/** Prepared prefix intervals plus rank minima return top matches without catalog traversal during lookup. */
import { requireValue } from '../../../core/src/required.js';
import { compareScalarText } from './scalar.js';
/** One prepared comparison key points to its immutable value and original rank. */
interface RankedEntry<T> {
  readonly key: string;
  readonly rank: number;
  readonly value: T;
}
/** Immutable range-minimum index; query cost depends on prefix length/result count and logarithmic catalog size. */
export class RankedPrefixIndex<T> {
  private readonly entries: readonly RankedEntry<T>[];
  private readonly minima: Int32Array;
  private readonly size: number;
  /** Capture original ranked values and build the separate lexical range index once. */
  constructor(ranked: readonly T[], key: (value: T) => string) {
    this.entries = ranked
      .map((value, rank) => ({
        key: key(value),
        rank,
        value,
      }))
      .sort((a, b) => compareScalarText(a.key, b.key));
    this.size = 2 ** Math.ceil(Math.log2(Math.max(1, ranked.length)));
    this.minima = new Int32Array(this.size * 2).fill(-1);
    for (let index = 0; index < this.entries.length; index++)
      this.minima[this.size + index] = index;
    for (let index = this.size - 1; index > 0; index--)
      this.minima[index] = this.best(
        requireValue(this.minima[index * 2], 'ranked-prefix'),
        requireValue(this.minima[index * 2 + 1], 'ranked-prefix'),
      );
  }
  /** Select the lower original rank, treating an empty range as absent. */
  private best(left: number, right: number): number {
    return left < 0
      ? right
      : right < 0
        ? left
        : requireValue(this.entries[left], 'ranked-prefix').rank <
            requireValue(this.entries[right], 'ranked-prefix').rank
          ? left
          : right;
  }
  /** Find the highest-ranked lexical entry in one half-open interval. */
  private minimum(from: number, to: number): number {
    let lower = from + this.size,
      upper = to + this.size,
      result = -1;
    while (lower < upper) {
      if (lower & 1)
        result = this.best(result, requireValue(this.minima[lower++], 'ranked-prefix'));
      if (upper & 1)
        result = this.best(result, requireValue(this.minima[--upper], 'ranked-prefix'));
      lower >>>= 1;
      upper >>>= 1;
    }
    return result;
  }
  /** Return the next ranked row from a bounded set of disjoint candidate intervals. */
  search(prefix: string, limit: number): readonly T[] {
    let lower = 0,
      upper = this.entries.length;
    while (lower < upper) {
      const middle = (lower + upper) >>> 1;
      if (compareScalarText(requireValue(this.entries[middle], 'ranked-prefix').key, prefix) < 0)
        lower = middle + 1;
      else upper = middle;
    }
    const from = lower;
    upper = this.entries.length;
    while (lower < upper) {
      const middle = (lower + upper) >>> 1;
      if (requireValue(this.entries[middle], 'ranked-prefix').key.startsWith(prefix))
        lower = middle + 1;
      else upper = middle;
    }
    const pending = [{ from, to: lower, best: this.minimum(from, lower) }];
    const result: T[] = [];
    while (result.length < limit && pending.length) {
      let chosen = -1,
        best = -1;
      for (let index = 0; index < pending.length; index++) {
        const candidate = requireValue(pending[index], 'ranked-prefix').best;
        if (candidate >= 0 && (best < 0 || this.best(best, candidate) === candidate)) {
          chosen = index;
          best = candidate;
        }
      }
      if (chosen < 0) break;
      const interval = requireValue(pending.splice(chosen, 1)[0], 'ranked-prefix');
      result.push(requireValue(this.entries[best], 'ranked-prefix').value);
      if (interval.from < best)
        pending.push({
          from: interval.from,
          to: best,
          best: this.minimum(interval.from, best),
        });
      if (best + 1 < interval.to)
        pending.push({
          from: best + 1,
          to: interval.to,
          best: this.minimum(best + 1, interval.to),
        });
    }
    return Object.freeze(result);
  }
}
