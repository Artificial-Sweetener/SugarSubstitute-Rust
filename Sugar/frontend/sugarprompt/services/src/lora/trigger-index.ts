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
/** Prepared lexical intervals avoid scanning model metadata during trigger-word prefix lookup. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold } from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';
import { compareScalarText } from '../text/scalar.js';
import { completionReplacementKey } from '../completion/replacement-key.js';
import type {
  LoraTriggerIndex,
  LoraTriggerSuggestion,
  ScheduledLora,
} from './scheduled-contracts.js';
/** Lookup spelling preserves interior whitespace and literal escapes; storage equivalence is derived separately. */
interface TriggerRow {
  readonly key: string;
  readonly replacementKey: string;
  readonly rank: number;
  readonly suggestion: LoraTriggerSuggestion;
}
/** Underscores and spaces are equivalent, without collapsing or trimming the typed prefix. */
function lookupKey(text: string): string {
  return caseFold(text.replaceAll('_', ' '));
}
/** Prepare every comma-split configured word once; matching precedes replacement deduplication as in the reference. */
export function prepareLoraTriggerIndex(models: readonly ScheduledLora[]): LoraTriggerIndex {
  const rows: TriggerRow[] = [];
  let units = 0;
  for (const model of models)
    for (const entry of model.triggerWords) {
      units += entry.length;
      if (units > 8000000) throw new TypeError('LoRA trigger index exceeds preparation budget');
      for (const part of entry.split(',')) {
        const word = strip(part);
        if (word) {
          if (rows.length === 100000)
            throw new TypeError('LoRA trigger index exceeds preparation budget');
          rows.push({
            key: lookupKey(word),
            replacementKey: completionReplacementKey(word),
            rank: rows.length,
            suggestion: Object.freeze({
              word,
              display: model.display,
              backendValue: model.model.backendValue,
            }),
          });
        }
      }
    }
  rows.sort((a, b) => compareScalarText(a.key, b.key) || a.rank - b.rank);
  const results = new Map<string, readonly LoraTriggerSuggestion[]>(),
    empty: readonly LoraTriggerSuggestion[] = Object.freeze([]);
  let retainedReferences = 0;
  return Object.freeze({
    /** Binary prefix bounds inspect only matching prepared rows; the last 32 normalized queries retain actual frozen output. */
    search(prefix: string): readonly LoraTriggerSuggestion[] {
      if (typeof prefix !== 'string' || prefix.length > 4096)
        throw new TypeError('Invalid LoRA trigger prefix');
      const key = lookupKey(prefix);
      if (!key) return empty;
      const cached = results.get(key);
      if (cached) {
        results.delete(key);
        results.set(key, cached);
        return cached;
      }
      let lower = 0,
        upper = rows.length;
      while (lower < upper) {
        const middle = (lower + upper) >>> 1;
        if (compareScalarText(requireValue(rows[middle], 'trigger-index').key, key) < 0)
          lower = middle + 1;
        else upper = middle;
      }
      const from = lower;
      upper = rows.length;
      while (lower < upper) {
        const middle = (lower + upper) >>> 1;
        if (requireValue(rows[middle], 'trigger-index').key.startsWith(key)) lower = middle + 1;
        else upper = middle;
      }
      const seen = new Set<string>(),
        matches = rows
          .slice(from, lower)
          .sort((a, b) => a.rank - b.rank)
          .filter((row) => {
            if (seen.has(row.replacementKey)) return false;
            seen.add(row.replacementKey);
            return true;
          })
          .map((row) => row.suggestion);
      const result = matches.length ? Object.freeze(matches) : empty;
      if (result.length <= 8192) {
        while (results.size === 32 || retainedReferences + result.length > 8192) {
          const oldest = requireValue(results.keys().next().value, 'trigger-index');
          retainedReferences -= requireValue(results.get(oldest), 'trigger-index').length;
          results.delete(oldest);
        }
        results.set(key, result);
        retainedReferences += result.length;
      }
      return result;
    },
  });
}
