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
/** One validated immutable catalog owns merge, ranking, prefix preparation and bounded result reuse. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold, normalizeTagIdentity, prepareTagLexicon } from '../../../core/src/index.js';
import { strip, whitespace } from '../../../core/src/prompt/text.js';
import type { TagCatalogInput, TagCompletionCatalog, TagSuggestion } from './contracts.js';
import { RankedPrefixIndex } from '../text/ranked-prefix.js';
import { compareScalarText, scalarTextLength } from '../text/scalar.js';
const empty: readonly TagSuggestion[] = Object.freeze([]);
/** Validate a closed decoded record without interpreting unexpected host fields. */
function record(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join() === [...keys].sort().join()
  );
}
/** Keep catalog text finite, single-line and independently bounded before preparing normalized identities. */
function tag(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    value.length <= 1024 &&
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    !/[\r\n\u0000]/u.test(value) &&
    normalizeTagIdentity(value).length > 0
  );
}
/** Read/copy host rows with a combined bound so no caller mutation can alter prepared lookup. */
function readRows(input: unknown): {
  revision: string;
  ranked: readonly TagSuggestion[];
} {
  if (
    !record(input, ['revision', 'bundled', 'customTags', 'censoredTags']) ||
    typeof input.revision !== 'string' ||
    input.revision.length < 1 ||
    input.revision.length > 256 ||
    !Array.isArray(input.bundled) ||
    !Array.isArray(input.customTags) ||
    !Array.isArray(input.censoredTags) ||
    input.bundled.length + input.customTags.length + input.censoredTags.length > 250000 ||
    !input.customTags.every(tag) ||
    !input.censoredTags.every(tag)
  )
    throw new TypeError('Invalid tag catalog');
  const bundled: TagSuggestion[] = [];
  for (const row of input.bundled as readonly unknown[]) {
    if (
      !record(row, ['tag', 'popularity']) ||
      !tag(row.tag) ||
      (row.popularity !== null &&
        (typeof row.popularity !== 'number' ||
          !Number.isSafeInteger(row.popularity) ||
          row.popularity < 0))
    )
      throw new TypeError('Invalid bundled tag metadata');
    bundled.push(
      Object.freeze({
        tag: strip(row.tag).replaceAll('_', ' '),
        popularity: row.popularity,
        sourceLabel: null,
      }),
    );
  }
  bundled.sort(
    (a, b) =>
      (b.popularity ?? 0) - (a.popularity ?? 0) ||
      scalarTextLength(a.tag) - scalarTextLength(b.tag) ||
      compareScalarText(caseFold(a.tag), caseFold(b.tag)),
  );
  const censored = new Set((input.censoredTags as string[]).map(normalizeTagIdentity));
  const seen = new Set<string>(),
    ranked: TagSuggestion[] = [];
  const custom = (input.customTags as string[]).map(
    (value): TagSuggestion =>
      Object.freeze({
        tag: strip(value.replaceAll('_', ' ')).replace(
          new RegExp(whitespace.source + '+', 'gu'),
          ' ',
        ),
        popularity: null,
        sourceLabel: 'custom',
      }),
  );
  for (const row of [...custom, ...bundled]) {
    const key = normalizeTagIdentity(row.tag);
    if (seen.has(key) || censored.has(key)) continue;
    seen.add(key);
    ranked.push(row);
  }
  return { revision: input.revision, ranked: Object.freeze(ranked) };
}
/** Validate/copy host data, prepare ranked lookup once and return a supported immutable catalog with matching lexicon. */
export function prepareTagCompletionCatalog(input: TagCatalogInput): TagCompletionCatalog {
  const prepared = readRows(input),
    index = new RankedPrefixIndex(prepared.ranked, (row) => normalizeTagIdentity(row.tag));
  const memo = new Map<string, readonly TagSuggestion[]>();
  return Object.freeze({
    revision: prepared.revision,
    lexicon: prepareTagLexicon(prepared.ranked.map((row) => row.tag)),
    /** Reuse actual immutable output for normalized prefix/limit identities; the bounded memo retains at most 64 queries. */
    search(prefix: string, limit = 10): readonly TagSuggestion[] {
      if (typeof prefix !== 'string' || !Number.isSafeInteger(limit) || limit < 0 || limit > 50)
        throw new TypeError('Invalid tag lookup');
      const key = normalizeTagIdentity(prefix);
      if (!key || !limit) return empty;
      const memoKey = JSON.stringify([key, limit]),
        cached = memo.get(memoKey);
      if (cached) {
        memo.delete(memoKey);
        memo.set(memoKey, cached);
        return cached;
      }
      const result = index.search(key, limit);
      memo.set(memoKey, result);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, 'catalog'));
      return result;
    },
  });
}
