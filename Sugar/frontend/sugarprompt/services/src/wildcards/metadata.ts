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
/** Validate, copy and normalize decoded wildcard metadata before preparing any lookup or search index. */
import { unicodeLowerCase } from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';
import type { WildcardCatalogInput } from './contracts.js';

/** Closed host record validation rejects accidental fields and malformed decoded arrays. */
function record(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join() === [...keys].sort().join()
  );
}

/** Host metadata text is bounded independently of its source representability. */
function text(value: unknown, bound: number): value is string {
  return typeof value === 'string' && value.length <= bound && !value.includes('\0');
}

/** Normalize relative POSIX paths as the reference catalog does, without folding distinct lowercase identities together. */
export function wildcardCatalogKey(identifier: string): string {
  const parts = identifier.split('/').filter((part) => part && part !== '.');
  if (
    !identifier ||
    identifier.startsWith('/') ||
    identifier.includes('\\') ||
    !parts.length ||
    parts.includes('..')
  )
    throw new TypeError('Invalid wildcard catalog identifier');
  return unicodeLowerCase(parts.join('/'));
}

/** One captured CSV identity retains first matching header spelling and prepared lowercase column lookup. */
export interface PreparedCsvMetadata {
  readonly columns: readonly string[];
  readonly lookup: ReadonlyMap<string, string>;
}

/** Metadata preparation is completed once before input/rendering; its maps are private to the catalog factory. */
export interface PreparedWildcardMetadata {
  readonly status: WildcardCatalogInput['status'];
  readonly revision: string;
  readonly textIdentifiers: ReadonlySet<string>;
  readonly csv: ReadonlyMap<string, PreparedCsvMetadata>;
}

/** Capture a closed, finite snapshot, merging duplicate roots in the host's explicit priority order. */
export function captureWildcardMetadata(value: unknown): PreparedWildcardMetadata {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new TypeError('Invalid wildcard catalog');
  const input = value as Record<string, unknown>;
  const ready = input.status === 'ready';
  if (
    (!ready && input.status !== 'pending' && input.status !== 'unavailable') ||
    !record(
      input,
      ready ? ['status', 'revision', 'textIdentifiers', 'csvSources'] : ['status', 'revision'],
    ) ||
    !text(input.revision, 256) ||
    !input.revision.length
  )
    throw new TypeError('Invalid wildcard catalog');
  const textIdentifiers = new Set<string>(),
    csv = new Map<string, PreparedCsvMetadata>(),
    builders = new Map<string, Map<string, string>>();
  let count = 0,
    length = 0;
  /** Bound aggregate text and rows before indexing, including duplicate host entries. */
  function capture(value: unknown): string {
    if (!text(value, 1024) || ++count > 250000 || (length += value.length) > 8_000_000)
      throw new TypeError('Wildcard catalog exceeds metadata bounds');
    return value;
  }
  if (ready) {
    if (
      !Array.isArray(input.textIdentifiers) ||
      !Array.isArray(input.csvSources) ||
      input.textIdentifiers.length + input.csvSources.length > 250000
    )
      throw new TypeError('Invalid wildcard catalog rows');
    for (const identifier of input.textIdentifiers as readonly unknown[])
      textIdentifiers.add(wildcardCatalogKey(capture(identifier)));
    for (const row of input.csvSources as readonly unknown[]) {
      if (
        !record(row, ['identifier', 'columns']) ||
        !Array.isArray(row.columns) ||
        row.columns.length > 250000
      )
        throw new TypeError('Invalid wildcard CSV metadata');
      const key = wildcardCatalogKey(capture(row.identifier)),
        lookup = builders.get(key) ?? new Map<string, string>();
      builders.set(key, lookup);
      for (const value of row.columns as readonly unknown[]) {
        const column = strip(capture(value));
        if (column && !lookup.has(unicodeLowerCase(column)))
          lookup.set(unicodeLowerCase(column), column);
      }
    }
  }
  for (const [key, lookup] of builders)
    if (lookup.size) csv.set(key, { columns: Object.freeze([...lookup.values()]), lookup });
  return {
    status: input.status as WildcardCatalogInput['status'],
    revision: input.revision,
    textIdentifiers,
    csv,
  };
}
