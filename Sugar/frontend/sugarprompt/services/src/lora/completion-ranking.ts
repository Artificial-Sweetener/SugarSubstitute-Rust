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
/** Scalar-aware ranking projects immutable catalog identities into the reference's deterministic ordering. */
import { caseFold } from '../../../core/src/index.js';
import type { LoraCatalogItem } from './contracts.js';
import type { LoraSuggestion, LoraMatchKind } from './contracts.js';
import { loraNameKey } from './names.js';
import { compareScalarText } from '../text/scalar.js';

/** Prepared normalized fields share one item identity and one immutable empty-query suggestion. */
export interface RankedLoraItem {
  readonly empty: LoraSuggestion;
  readonly basename: string;
  readonly display: string;
  readonly name: string;
  readonly backend: string;
  readonly metadata: string;
  readonly displaySort: string;
  readonly pathSort: string;
}

/** Copy only derived comparison fields; host descriptors were already validated/captured by the catalog owner. */
export function rankedLoraItem(
  item: LoraCatalogItem,
  metadata: string,
  scheduleName: string,
): RankedLoraItem {
  const display = item.title || item.model.basename;
  return Object.freeze({
    empty: Object.freeze({
      item,
      scheduleName,
      score: 900,
      matchKind: 'empty',
      display,
    }),
    basename: loraNameKey(item.model.basename),
    display: loraNameKey(item.title),
    name: loraNameKey(item.model.promptName),
    backend: loraNameKey(item.model.backendValue, true),
    metadata: loraNameKey(metadata),
    displaySort: caseFold(display),
    pathSort: caseFold(item.model.backendValue),
  });
}

/** A UTF-16 match boundary cannot split an astral scalar when either input contains isolated surrogates. */
function scalarBoundary(text: string, position: number): boolean {
  const before = text.charCodeAt(position - 1),
    after = text.charCodeAt(position);
  return !(before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff);
}

/** Match complete scalar substrings, preserving Python behavior for malformed UTF-16 host strings. */
function contains(text: string, query: string, prefix = false): boolean {
  for (
    let position = text.indexOf(query);
    position >= 0;
    position = text.indexOf(query, position + 1)
  ) {
    if (prefix && position !== 0) return false;
    if (scalarBoundary(text, position) && scalarBoundary(text, position + query.length))
      return true;
  }
  return false;
}

/** Score one posting candidate with the original priority; installed filenames match exactly after extension removal. */
export function rankLora(item: RankedLoraItem, query: string): LoraSuggestion | null {
  if (!query) return item.empty;
  /** Retain literal metadata and identity without creating another mutable catalog record. */
  const result = (score: number, matchKind: LoraMatchKind): LoraSuggestion =>
    Object.freeze({ ...item.empty, score, matchKind });
  if ([item.name, item.backend, item.display, item.basename].includes(query))
    return result(0, 'exact');
  if (contains(item.basename, query, true)) return result(100, 'basename_prefix');
  if (contains(item.display, query, true)) return result(110, 'display_prefix');
  if (query.includes('/') && contains(item.name, query, true)) return result(120, 'path_prefix');
  if (contains(item.basename, query)) return result(300, 'basename_substring');
  if (contains(item.display, query)) return result(310, 'display_substring');
  if (contains(item.name, query)) return result(330, 'path_substring');
  return contains(item.metadata, query) ? result(700, 'metadata') : null;
}

/** Stable ordering preserves input ties after score, full-folded friendly display and literal path spelling. */
export function compareRankedLoras(left: RankedLoraItem, right: RankedLoraItem): number {
  return (
    compareScalarText(left.displaySort, right.displaySort) ||
    compareScalarText(left.pathSort, right.pathSort)
  );
}
