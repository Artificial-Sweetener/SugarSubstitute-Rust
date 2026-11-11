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
/** Host workflow titles prepare one immutable bounded prefix catalog; no workflow graph or editor source is retained. */
import { requireValue } from '../../../core/src/required.js';
import { caseFold, isSceneTitle } from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';
import { RankedPrefixIndex } from '../text/ranked-prefix.js';
/** Ready host authority supplies literal titles; pending and unavailable publications are explicit. */
export type SceneTitleCatalogInput =
  | {
      readonly status: 'ready';
      readonly revision: string;
      readonly titles: readonly string[];
    }
  | {
      readonly status: 'pending' | 'unavailable';
      readonly revision: string;
    };
/** Host metadata is source-independent and replaced by identity; lookup reuses bounded prepared output. */
export interface SceneTitleCatalog {
  readonly status: SceneTitleCatalogInput['status'];
  readonly revision: string;
  /** Return first-authored, stripped/case-folded prefix matches in workflow order, excluding complete titles. */
  search(prefix: string, limit?: number): readonly string[];
}
const empty: readonly string[] = Object.freeze([]);
/** Validate a closed host publication and capture literal titles before building their separate comparison index. */
export function prepareSceneTitleCatalog(input: SceneTitleCatalogInput): SceneTitleCatalog {
  if (
    !input ||
    typeof input !== 'object' ||
    Array.isArray(input) ||
    !['ready', 'pending', 'unavailable'].includes(input.status) ||
    typeof input.revision !== 'string' ||
    !input.revision ||
    input.revision.length > 256 ||
    Object.keys(input).sort().join() !==
      (input.status === 'ready' ? 'revision,status,titles' : 'revision,status')
  )
    throw new TypeError('Invalid scene title catalog');
  const titles: string[] = [],
    seen = new Set<string>();
  if (input.status === 'ready') {
    if (!Array.isArray(input.titles) || input.titles.length > 10000)
      throw new TypeError('Invalid scene title rows');
    for (const title of input.titles) {
      if (typeof title !== 'string' || title.length > 4096)
        throw new TypeError('Invalid scene title row');
      if (!strip(title)) continue;
      if (!isSceneTitle(title)) throw new TypeError('Invalid scene title row');
      const key = caseFold(strip(title));
      if (!seen.has(key)) {
        seen.add(key);
        titles.push(title);
      }
    }
  }
  const { status, revision } = input;
  const index = new RankedPrefixIndex(titles, (title) => caseFold(strip(title)));
  const memo = new Map<string, readonly string[]>();
  return Object.freeze({
    status,
    revision,
    /** Prefix/limit memoization is bounded to 64 immutable results; context replacement retires its complete owner. */
    search(prefix: string, limit = 10): readonly string[] {
      if (typeof prefix !== 'string' || !Number.isSafeInteger(limit) || limit < 0 || limit > 50)
        throw new TypeError('Invalid scene title lookup');
      if (status !== 'ready' || !limit) return empty;
      const key = caseFold(strip(prefix)),
        identity = JSON.stringify([key, limit]);
      const reused = memo.get(identity);
      if (reused) {
        memo.delete(identity);
        memo.set(identity, reused);
        return reused;
      }
      const matches = Object.freeze(
        index
          .search(key, limit + 1)
          .filter((title) => !key || caseFold(strip(title)) !== key)
          .slice(0, limit),
      );
      memo.set(identity, matches);
      if (memo.size > 64) memo.delete(requireValue(memo.keys().next().value, 'scene-catalog'));
      return matches;
    },
  });
}
