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
/** Captured wildcard metadata has one prepared service owner; hosts acquire files and CSV headers. */
import type { WildcardSpan } from '../../../core/src/index.js';

/** A relative POSIX identifier without extension and ordered decoded CSV headers from one host source. */
export interface WildcardCsvSource {
  readonly identifier: string;
  readonly columns: readonly string[];
}

/** Ready sources arrive in host priority order; first case-insensitive header spelling wins across sources. */
export type WildcardCatalogInput =
  | { readonly status: 'pending' | 'unavailable'; readonly revision: string }
  | {
      readonly status: 'ready';
      readonly revision: string;
      readonly textIdentifiers: readonly string[];
      readonly csvSources: readonly WildcardCsvSource[];
    };

/** Exact authored identity remains independent of catalog lowercase lookup and displayed group identity. */
export type WildcardReference = Pick<WildcardSpan, 'form' | 'identifier' | 'csvColumn'>;

/** Confirmed missing results distinguish files and columns; incomplete snapshots never report absence. */
export type WildcardResolution =
  | { readonly status: 'pending' }
  | { readonly status: 'unavailable' }
  | {
      readonly status: 'missing';
      readonly reason: 'source' | 'column';
      readonly availableColumns: readonly string[];
    }
  | {
      readonly status: 'found';
      readonly matchedCsvColumn: string | null;
      readonly availableColumns: readonly string[];
    };

/** Literal source body suggestion; wrapper choice and whole-placeholder replacement belong to core/editor acceptance. */
export interface WildcardSuggestion {
  readonly tag: string;
  readonly sourceLabel: 'TXT wildcard' | 'CSV wildcard';
}

/** Prepared search preserves incomplete data states instead of treating them as an empty completed query. */
export type WildcardSearchResult =
  | { readonly status: 'pending' }
  | { readonly status: 'unavailable' }
  | {
      readonly status: 'ready';
      readonly suggestions: readonly WildcardSuggestion[];
    };

/** Immutable metadata revision; replacement installs a new prepared object and retires editor command freshness. */
export interface WildcardCatalog {
  readonly revision: string;
  readonly status: WildcardCatalogInput['status'];
  /** Resolve an authored reference with pinned lowercase identities, preserving the canonical CSV header display. */
  resolve(reference: WildcardReference): WildcardResolution;
  /** Search with pinned full case folding and reference score/order; at most 50 rows, default 10. */
  search(prefix: string, limit?: number): WildcardSearchResult;
}
