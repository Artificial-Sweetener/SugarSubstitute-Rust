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
/** Catalog orchestration delegates source matching to core and uses last-word fallback only after a true full miss. */
import { tagIsNoop, type TagQuery } from '../../../core/src/index.js';
import type {
  TagCompletionCatalog,
  TagCompletionResult,
  PromptTextCompletionResult,
  PromptTextSuggestion,
} from './contracts.js';
import type { LoraTriggerIndex } from '../lora/scheduled-contracts.js';
import { completionReplacementKey } from './replacement-key.js';

/** Return one concrete prepared result; no-op-only candidates suppress fallback just as the pinned result owner does. */
export function findTagCompletions(
  catalog: TagCompletionCatalog,
  source: string,
  query: TagQuery,
  limit = 10,
): TagCompletionResult {
  const result = findPromptTextCompletions(catalog, null, source, query, limit);
  return Object.freeze({
    query: result.query,
    hadCandidates: result.hadCandidates,
    matches: Object.freeze(
      result.matches.flatMap((row) => (row.kind === 'tag' ? [row.value] : [])),
    ),
  });
}

/** Merge trigger-first metadata before limiting/no-op removal, promoting fallback only after a true full miss.
 * The ordinary-tag public API delegates here with no trigger index; both paths share one behavior owner.
 */
export function findPromptTextCompletions(
  catalog: TagCompletionCatalog | null,
  triggers: LoraTriggerIndex | null,
  source: string,
  query: TagQuery,
  limit = 10,
): PromptTextCompletionResult {
  if (!Number.isSafeInteger(limit) || limit < 0 || limit > 50)
    throw new RangeError('Invalid prompt completion limit');
  const candidates: PromptTextSuggestion[] = [],
    seen = new Set<string>();
  /** Capture only visible ranked alternatives, without copying every prepared trigger match. */
  function add(row: PromptTextSuggestion, word: string): void {
    const key = completionReplacementKey(word);
    if (seen.has(key)) return;
    seen.add(key);
    candidates.push(Object.freeze(row));
  }
  for (const value of triggers?.search(query.prefix) ?? []) {
    if (candidates.length === limit) break;
    add({ kind: 'trigger', value }, value.word);
  }
  if (candidates.length < limit)
    for (const value of catalog?.search(query.prefix, limit) ?? []) {
      if (candidates.length === limit) break;
      add({ kind: 'tag', value }, value.tag);
    }
  const result = Object.freeze({
    query,
    hadCandidates: candidates.length > 0,
    matches: Object.freeze(
      candidates.filter(
        (row) => !tagIsNoop(source, query, row.kind === 'tag' ? row.value.tag : row.value.word),
      ),
    ),
  });
  if (result.hadCandidates || !query.fallback) return result;
  const fallback = findPromptTextCompletions(catalog, triggers, source, query.fallback, limit);
  return fallback.matches.length ? fallback : result;
}
