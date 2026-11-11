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
/** Prepared host catalogs implement the supported adapter without consumer-owned matching or private scheduling machinery. */
import type { LoraCatalogSnapshot, LoraResource, LoraResourceAdapter } from './contracts.js';
import { readLoraCatalog } from './catalog-validation.js';
import { LoraCatalogIndex } from './catalog-index.js';
import { prepareLoraCompletion } from './completion-catalog.js';
import type { LoraCompletionOptions } from './contracts.js';

/** Validate/copy one immutable host catalog, build derived indexes once and expose exact-name resource resolution.
 * The host retains media URL ownership. Replace this adapter for catalog, authority or media changes.
 * Explicit completion options add ranked suggestions over the same captured model metadata.
 */
export function createLoraCatalogAdapter(
  input: LoraCatalogSnapshot,
  completion?: LoraCompletionOptions,
): LoraResourceAdapter {
  const catalog = readLoraCatalog(input),
    index = new LoraCatalogIndex(catalog.items);
  return Object.freeze({
    revision: catalog.revision,
    ...(completion === undefined
      ? {}
      : {
          completionCatalog: prepareLoraCompletion(catalog, completion, index),
        }),
    /** Resolve current names from prepared indexes; cancelled requests have no external work or publication side effects. */
    async resolve(names: readonly string[], signal: AbortSignal): Promise<readonly LoraResource[]> {
      if (
        !Array.isArray(names) ||
        names.some((name) => typeof name !== 'string' || !name.length) ||
        new Set(names).size !== names.length
      )
        throw new TypeError('Invalid LoRA lookup names');
      return Object.freeze(
        names.map((name): LoraResource => {
          if (signal.aborted) return Object.freeze({ name, resolution: 'unavailable' });
          if (!catalog.authoritative)
            return Object.freeze({ name, resolution: 'pending-authority' });
          const result = index.find(name);
          if (result.status === 'found')
            return Object.freeze({ name, resolution: 'found', ...result.item });
          return result.status === 'ambiguous'
            ? Object.freeze({
                name,
                resolution: 'ambiguous',
                candidateCount: result.candidateCount,
              })
            : Object.freeze({ name, resolution: 'missing' });
        }),
      );
    },
  });
}
