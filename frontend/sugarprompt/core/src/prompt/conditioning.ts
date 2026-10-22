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
/** Parsed partitions own exact-source UTF-16 conditioning ranges; host classification determines inheritance. */
import { requireValue } from '../required.js';
import type { RegionStructure } from './contracts.js';
import type { SourceRange } from '../offsets.js';
/** Unresolved host authority retains independent conditions instead of guessing regional inheritance. */
export type ConditioningMode = 'unresolved' | 'independent' | 'regional';
/** One canonical partition contributes local source and, for a regional child, its global parent. */
export interface ConditioningScope {
  readonly id: string;
  readonly partition: number;
  readonly role: 'condition' | 'global' | 'region';
  readonly range: SourceRange;
  readonly parent: string | null;
  readonly emits: boolean;
}
/** Immutable prepared scope lookups reuse parsed ownership without interpreting separator syntax again. */
export class ConditioningTopology {
  readonly mode: ConditioningMode;
  readonly scopes: readonly ConditioningScope[];
  readonly roots: readonly ConditioningScope[];
  readonly emitted: readonly ConditioningScope[];
  private readonly identities: ReadonlyMap<string, ConditioningScope>;
  private readonly inherited: ReadonlyMap<string, readonly SourceRange[]>;
  private readonly emptyChildren: readonly ConditioningScope[] = Object.freeze([]);
  /** Derive one topology from canonical source partitions, including leading, adjacent and trailing empties. */
  constructor(mode: ConditioningMode, regions: RegionStructure) {
    this.mode = mode;
    const regional = mode === 'regional' && regions.separators.length > 0;
    this.scopes = Object.freeze(
      regions.partitions.map((partition) =>
        Object.freeze({
          id: `partition:${partition.index}`,
          partition: partition.index,
          role: regional
            ? partition.index === 0
              ? ('global' as const)
              : ('region' as const)
            : ('condition' as const),
          range: Object.freeze({ ...partition.sourceRange }),
          parent: regional && partition.index !== 0 ? 'partition:0' : null,
          emits: !regional || partition.index !== 0,
        }),
      ),
    );
    this.roots = Object.freeze(this.scopes.filter((scope) => scope.parent === null));
    this.emitted = Object.freeze(this.scopes.filter((scope) => scope.emits));
    this.identities = new Map(this.scopes.map((scope) => [scope.id, scope]));
    this.inherited = new Map(
      this.scopes.map((scope) => [
        scope.id,
        Object.freeze(scope.parent ? [this.scope(scope.parent).range, scope.range] : [scope.range]),
      ]),
    );
    Object.freeze(this);
  }
  /** Resolve a stable scope identity; unknown ownership rejects rather than returning an unrelated partition. */
  scope(id: string): ConditioningScope {
    const scope = this.identities.get(id);
    if (!scope) throw new RangeError('Unknown conditioning scope');
    return scope;
  }
  /** Resolve one zero-based canonical source partition. */
  partition(index: number): ConditioningScope {
    return this.scope(`partition:${index}`);
  }
  /** Binary source lookup excludes separator lines, empty intervals and the terminal source boundary. */
  at(position: number): ConditioningScope | null {
    let from = 0,
      to = this.scopes.length;
    while (from < to) {
      const middle = (from + to) >>> 1;
      if (requireValue(this.scopes[middle], 'conditioning').range.from <= position)
        from = middle + 1;
      else to = middle;
    }
    const scope = this.scopes[from - 1];
    return scope && scope.range.from <= position && position < scope.range.to ? scope : null;
  }
  /** Source-ordered direct children share immutable prepared scope objects. */
  children(id: string): readonly ConditioningScope[] {
    this.scope(id);
    return id === 'partition:0' && this.scopes[0]?.role === 'global'
      ? this.emitted
      : this.emptyChildren;
  }
  /** Reuse prepared inherited/local exact-source UTF-16 ranges; siblings never contribute to one another. */
  effectiveRanges(id: string): readonly SourceRange[] {
    this.scope(id);
    return requireValue(this.inherited.get(id), 'conditioning');
  }
}
