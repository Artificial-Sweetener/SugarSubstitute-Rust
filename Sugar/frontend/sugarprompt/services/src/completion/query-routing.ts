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
/** One stateless router selects a syntax owner from source-current core indexes and resolved feature permissions. */
import type {
  PreparedLoraQueries,
  PreparedWildcardQueries,
  PreparedSceneQueries,
  PreparedTagQueries,
  LoraQuery,
  WildcardQuery,
  SceneQuery,
  TagQuery,
} from '../../../core/src/index.js';
import type { FeatureProfile } from '../features/contracts.js';
import { featureEnabled } from '../features/profile.js';

/** Core preparation retains source and bounds; a null scene index explicitly excludes title authoring. */
export interface CompletionQueryIndexes {
  readonly loraQueries: PreparedLoraQueries;
  readonly wildcardQueries: PreparedWildcardQueries;
  readonly sceneQueries: PreparedSceneQueries | null;
  readonly tagQueries: PreparedTagQueries;
}

/** Each selected owner retains its actual core-issued query with exact-source UTF-16 ranges and distinct acceptance semantics. */
export type RoutedCompletionQuery =
  | { readonly kind: 'lora'; readonly query: LoraQuery }
  | { readonly kind: 'wildcard'; readonly query: WildcardQuery }
  | { readonly kind: 'scene'; readonly query: SceneQuery }
  | { readonly kind: 'tag'; readonly query: TagQuery };

/** Resolve the first enabled source slot at an exact-source UTF-16 boundary; catalog availability never changes syntax ownership. */
export function resolveCompletionQuery(
  indexes: CompletionQueryIndexes,
  profile: FeatureProfile,
  position: number,
  hasSelection: boolean,
): RoutedCompletionQuery | null {
  if (featureEnabled(profile, 'lora_autocomplete')) {
    const query = indexes.loraQueries.queryAt(position, hasSelection);
    if (query) return Object.freeze({ kind: 'lora', query });
  }
  if (featureEnabled(profile, 'wildcard_autocomplete')) {
    const query = indexes.wildcardQueries.queryAt(position, hasSelection);
    if (query) return Object.freeze({ kind: 'wildcard', query });
  }
  const scene = indexes.sceneQueries?.queryAt(position, hasSelection);
  if (scene) return Object.freeze({ kind: 'scene', query: scene });
  const tag = indexes.tagQueries.queryAt(position, hasSelection);
  return tag ? Object.freeze({ kind: 'tag', query: tag }) : null;
}
