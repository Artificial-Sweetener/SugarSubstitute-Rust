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
/** Prepared metadata owns lowercase resolution, folded ranked search and bounded reusable output. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold, unicodeLowerCase, isWildcardCompletionBody } from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';
import { ScalarSubstringIndex } from '../text/substring-index.js';
import { compareScalarText } from '../text/scalar.js';
import { captureWildcardMetadata, wildcardCatalogKey } from './metadata.js';
import type {
  WildcardCatalog,
  WildcardCatalogInput,
  WildcardResolution,
  WildcardSearchResult,
  WildcardSuggestion,
  WildcardReference,
} from './contracts.js';
const noColumns: readonly string[] = Object.freeze([]);
const noMatches: WildcardSearchResult = Object.freeze({
  status: 'ready',
  suggestions: Object.freeze([]),
});
/** Prepared search fields retain a stable tie key and source kind independently of label spelling. */
interface SearchRow {
  readonly identifier: string;
  readonly foldedTag: string;
  readonly key: string;
  readonly csv: boolean;
  readonly suggestion: WildcardSuggestion;
}
/** Pinned rank priorities distinguish identifier matches, literal CSV activators and contained text. */
function score(row: SearchRow, prefix: string): number | null {
  if (!prefix) return 100;
  if (row.identifier.startsWith(prefix)) return 0;
  if (row.foldedTag.startsWith(prefix)) return 20;
  if (row.identifier.includes(prefix)) return 200;
  if (row.foldedTag.includes(prefix)) return 220;
  return null;
}
/** Validate/capture host metadata and build one supported immutable catalog; no filesystem, network or editor state is retained. */
export function prepareWildcardCatalog(input: WildcardCatalogInput): WildcardCatalog {
  const metadata = captureWildcardMetadata(input),
    incomplete = metadata.status === 'ready' ? null : Object.freeze({ status: metadata.status });
  const rows: SearchRow[] = [];
  /** Omit unrepresentable bodies before ranking and limits; acceptance validates the selected wrapper again. */
  function add(identifier: string, tag: string, csv: boolean): void {
    if (!isWildcardCompletionBody(tag, csv ? 'csv' : 'simple')) return;
    rows.push({
      identifier: caseFold(identifier),
      foldedTag: caseFold(tag),
      key: csv ? caseFold(tag) : identifier,
      csv,
      suggestion: Object.freeze({
        tag,
        sourceLabel: csv ? 'CSV wildcard' : 'TXT wildcard',
      }),
    });
  }
  for (const identifier of [...metadata.textIdentifiers].sort(compareScalarText))
    add(identifier, identifier, false);
  for (const [identifier, entry] of [...metadata.csv].sort((a, b) => compareScalarText(a[0], b[0])))
    for (const column of entry.columns) add(identifier, `csv:${identifier}:${column}`, true);
  const index = new ScalarSubstringIndex(rows, (row) => [row.identifier, row.foldedTag]),
    memo = new Map<string, WildcardSearchResult>();
  return Object.freeze({
    revision: metadata.revision,
    status: metadata.status,
    /** Only completed snapshots establish missing state; CSV spelling is captured at preparation. */
    resolve(reference: WildcardReference): WildcardResolution {
      if (
        !reference ||
        (reference.form !== 'simple' && reference.form !== 'csv') ||
        typeof reference.identifier !== 'string' ||
        reference.identifier.length > 1024 ||
        (reference.form === 'simple'
          ? reference.csvColumn !== null
          : typeof reference.csvColumn !== 'string' || reference.csvColumn.length > 1024)
      )
        throw new TypeError('Invalid wildcard reference');
      if (incomplete) return incomplete;
      const key = wildcardCatalogKey(reference.identifier);
      if (reference.form === 'simple')
        return metadata.textIdentifiers.has(key)
          ? Object.freeze({
              status: 'found',
              matchedCsvColumn: null,
              availableColumns: noColumns,
            })
          : Object.freeze({
              status: 'missing',
              reason: 'source',
              availableColumns: noColumns,
            });
      const csv = metadata.csv.get(key),
        matched = csv?.lookup.get(
          unicodeLowerCase(strip(requireValue(reference.csvColumn, 'catalog'))),
        );
      return matched !== undefined
        ? Object.freeze({
            status: 'found',
            matchedCsvColumn: matched,
            availableColumns: requireValue(csv, 'catalog').columns,
          })
        : Object.freeze({
            status: 'missing',
            reason: csv ? 'column' : 'source',
            availableColumns: csv?.columns ?? noColumns,
          });
    },
    /** One immutable result per folded prefix/limit, capped at 64 entries/3200 suggestion references. */
    search(prefix: string, limit = 10): WildcardSearchResult {
      if (
        typeof prefix !== 'string' ||
        prefix.length > 4096 ||
        !Number.isSafeInteger(limit) ||
        limit < 0 ||
        limit > 50
      )
        throw new TypeError('Invalid wildcard search');
      if (incomplete) return incomplete;
      if (!limit) return noMatches;
      const folded = caseFold(prefix),
        csvOnly = folded.startsWith('csv:'),
        query = csvOnly ? folded.slice(4) : folded,
        key = JSON.stringify([folded, limit]),
        cached = memo.get(key);
      if (cached) {
        memo.delete(key);
        memo.set(key, cached);
        return cached;
      }
      const matches = index.candidates(query).flatMap((row) => {
        if (csvOnly && !row.csv) return [];
        const rank = score(row, query);
        return rank === null ? [] : [{ row, rank: rank + (row.csv ? 10 : 0) }];
      });
      matches.sort((a, b) => a.rank - b.rank || compareScalarText(a.row.key, b.row.key));
      const result: WildcardSearchResult = Object.freeze({
        status: 'ready',
        suggestions: Object.freeze(matches.slice(0, limit).map((match) => match.row.suggestion)),
      });
      memo.set(key, result);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, 'catalog'));
      return result;
    },
  });
}
