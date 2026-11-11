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
/** Prepared scalar gram postings avoid catalog traversal for ordinary substring queries. */

/** Generate one-, two- and three-scalar postings, with no matches spanning separate searchable fields. */
function grams(value: string, size: number): readonly string[] {
  const characters = Array.from(value),
    result: string[] = [];
  for (let start = 0; start <= characters.length - size; start++)
    result.push(characters.slice(start, start + size).join(''));
  return result;
}

/** One immutable index returns the smallest possible posting to the exact ranking owner. */
export class ScalarSubstringIndex<T> {
  private readonly postings = new Map<string, readonly T[]>();
  private readonly items: readonly T[];

  /** Build once from bounded captured fields, deduplicating each item across metadata and identity aliases. */
  constructor(items: readonly T[], fields: (item: T) => readonly string[]) {
    this.items = Object.freeze([...items]);
    const postings = new Map<string, T[]>();
    for (const item of items) {
      const keys = new Set<string>();
      for (const field of fields(item)) {
        for (const size of [1, 2, 3])
          for (const gram of grams(field, size)) keys.add(`${size}:${gram}`);
      }
      for (const key of keys) {
        const posting = postings.get(key);
        if (posting) posting.push(item);
        else postings.set(key, [item]);
      }
    }
    for (const [key, values] of postings) this.postings.set(key, Object.freeze(values));
  }

  /** Empty queries deliberately include every model; other lookups use the rarest scalar gram, with final exact matching downstream. */
  candidates(query: string): readonly T[] {
    if (!query) return this.items;
    const size = Math.min(3, Array.from(query).length);
    let selected: readonly T[] | null = null;
    for (const gram of grams(query, size)) {
      const posting = this.postings.get(`${size}:${gram}`);
      if (!posting) return [];
      if (!selected || posting.length < selected.length) selected = posting;
    }
    return selected ?? [];
  }
}
