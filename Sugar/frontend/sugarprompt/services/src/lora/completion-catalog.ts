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
/** One captured catalog prepares ranked substring search and bounded reusable output independently of editor input. */
import { requireValue } from '../../../core/src/required.js';
import { loraScheduleText } from '../../../core/src/index.js';
import type { LoraCatalogSnapshot, LoraCatalogItem } from './contracts.js';
import type { LoraCatalogIndex } from './catalog-index.js';
import type { LoraCompletionCatalog, LoraSuggestion, LoraCompletionOptions } from './contracts.js';
import { loraRecord, loraText } from './metadata-validation.js';
import { loraNameKey } from './names.js';
import {
  rankedLoraItem,
  rankLora,
  compareRankedLoras,
  type RankedLoraItem,
} from './completion-ranking.js';
import { ScalarSubstringIndex } from '../text/substring-index.js';
/** Validate explicit host search metadata by exact installed identity; default text comes only from prepared public descriptors. */
function searchMetadata(input: unknown, catalog: LoraCatalogSnapshot): ReadonlyMap<string, string> {
  const raw = loraRecord(input, ['searchMetadata']);
  if (!Array.isArray(raw.searchMetadata) || raw.searchMetadata.length > catalog.items.length)
    throw new TypeError('Invalid LoRA completion metadata');
  const identities = new Set(catalog.items.map((item) => item.model.backendValue)),
    result = new Map<string, string>();
  for (const value of raw.searchMetadata as readonly unknown[]) {
    const record = loraRecord(value, ['backendValue', 'text']),
      identity = loraText(record.backendValue, 4096),
      text = loraText(record.text, 16384, true);
    if (!identities.has(identity) || result.has(identity))
      throw new TypeError('Unknown or duplicate LoRA completion identity');
    result.set(identity, text);
  }
  return result;
}
/** Delegate identity decisions to the shared resolution index; never publish a completion that selects a different or ambiguous model. */
function schedulingName(item: LoraCatalogItem, index: LoraCatalogIndex): string {
  loraScheduleText(item.model.promptName);
  for (const name of [item.model.promptName, item.model.backendValue]) {
    const match = index.find(name);
    if (match.status === 'found' && match.item === item) {
      loraScheduleText(name);
      return name;
    }
  }
  throw new TypeError('LoRA completion identity is not source-representable');
}
/** Prepare from the factory's captured catalog and resolution index; source spelling and metadata refer to the same installed model. */
export function prepareLoraCompletion(
  catalog: LoraCatalogSnapshot,
  options: LoraCompletionOptions,
  resolution: LoraCatalogIndex,
): LoraCompletionCatalog {
  const metadata = searchMetadata(options, catalog),
    items: RankedLoraItem[] = [];
  let total = 0;
  for (const item of catalog.items) {
    const ranked = rankedLoraItem(
      item,
      metadata.get(item.model.backendValue) ??
        [item.title, item.subtitle ?? '', ...item.triggerWords].join(' '),
      schedulingName(item, resolution),
    );
    total +=
      ranked.basename.length +
      ranked.display.length +
      ranked.name.length +
      ranked.backend.length +
      ranked.metadata.length;
    if (total > 8000000) throw new TypeError('LoRA completion catalog exceeds prepared text bound');
    items.push(ranked);
  }
  Object.freeze(items);
  const index = new ScalarSubstringIndex(items, (item) => [
      item.basename,
      item.display,
      item.name,
      item.backend,
      item.metadata,
    ]),
    ordered = Object.freeze([...items].sort(compareRankedLoras).map((item) => item.empty)),
    memo = new Map<string, readonly LoraSuggestion[]>();
  let retained = 0;
  return Object.freeze({
    revision: catalog.revision,
    authoritative: catalog.authoritative,
    /** Lookup returns every match, with at most 64 queries/8192 candidate references retained for actual output reuse. */
    search(query: string): readonly LoraSuggestion[] {
      if (typeof query !== 'string' || query.length > 4096)
        throw new TypeError('Invalid LoRA completion query');
      const key = loraNameKey(query);
      if (!key) return ordered;
      const cached = memo.get(key);
      if (cached) {
        memo.delete(key);
        memo.set(key, cached);
        return cached;
      }
      const ranked = index.candidates(key).flatMap((item) => {
        const result = rankLora(item, key);
        return result ? [{ item, result }] : [];
      });
      ranked.sort((a, b) => a.result.score - b.result.score || compareRankedLoras(a.item, b.item));
      const result = Object.freeze(ranked.map((row) => row.result));
      if (result.length <= 8192) {
        memo.set(key, result);
        retained += result.length;
        while (memo.size > 64 || retained > 8192) {
          const oldest = requireValue(memo.keys().next().value, 'completion-catalog');
          retained -= requireValue(memo.get(oldest), 'completion-catalog').length;
          memo.delete(oldest);
        }
      }
      return result;
    },
  });
}
