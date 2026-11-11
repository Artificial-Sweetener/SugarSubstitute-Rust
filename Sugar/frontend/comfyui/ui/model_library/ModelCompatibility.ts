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
/** Query prepared metadata without duplicating provider relationship or compatibility rules. */
import type { ModelCatalogItem } from './ModelCatalog.js';
import type { ModelRequirement } from '../cube/ModelRequirement.js';

export interface ModelCompatibilityContext extends ModelRequirement {
  readonly addon: boolean;
  readonly label: string;
}

export interface ModelCatalogQuery {
  readonly architecture?: string;
  readonly family?: string;
  readonly nsfw?: 'sfw' | 'nsfw' | 'unknown';
  readonly kind?: string;
  readonly tags?: readonly string[];
  readonly text?: string;
  readonly compatibleWith?: ModelCompatibilityContext | null;
}

/** Use server decisions for addons and exact declared architecture/family requirements for models. */
export function modelCompatibility(
  item: ModelCatalogItem,
  context: ModelCompatibilityContext,
): 'full' | 'partial' | 'incompatible' | 'unknown' {
  const traits = item.traits;
  if (traits?.conflict) return 'unknown';
  const architecture = traits?.architecture || item.architecture?.base || '';
  if (context.addon && context.family) {
    const support = traits?.compatibleTargets.find(
      (target) => target.family === context.family,
    )?.support;
    return support ?? (traits?.family ? 'incompatible' : 'unknown');
  }
  if (context.architecture && !architecture) return 'unknown';
  if (context.architecture && architecture !== context.architecture) return 'incompatible';
  if (context.family && !traits?.family) return 'unknown';
  if (context.family && traits?.family !== context.family) return 'incompatible';
  // Architecture alone cannot establish release-specific addon compatibility.
  return context.addon ? 'unknown' : 'full';
}

/** Expose filters for existing pickers and a future browser without creating a browser surface. */
export function queryModelCatalog(
  items: readonly ModelCatalogItem[],
  query: ModelCatalogQuery,
): readonly ModelCatalogItem[] {
  const terms = query.text?.trim().toLowerCase().split(/\s+/).filter(Boolean) ?? [];
  return items.filter(
    (item) =>
      (!query.kind || item.kind === query.kind) &&
      (!query.architecture ||
        (item.traits?.architecture || item.architecture?.base) === query.architecture) &&
      (!query.family || item.traits?.family === query.family) &&
      (!query.nsfw || (item.traits?.nsfw ?? 'unknown') === query.nsfw) &&
      (!query.tags ||
        query.tags.every((tag) =>
          item.tags?.some((value) => value.toLowerCase() === tag.toLowerCase()),
        )) &&
      terms.every((term) =>
        [
          item.value,
          item.title,
          item.baseModel,
          item.traits?.familyName ?? '',
          ...item.trainedWords,
          ...(item.tags ?? []),
        ]
          .join(' ')
          .toLowerCase()
          .includes(term),
      ) &&
      (!query.compatibleWith ||
        ['full', 'partial'].includes(modelCompatibility(item, query.compatibleWith))),
  );
}

/** Preserve a compatible selection; choose a full match before a partial match for missing models. */
export function resolveInstalledModel(
  items: readonly ModelCatalogItem[],
  current: string,
  context: ModelCompatibilityContext,
): ModelCatalogItem | null {
  const compatible = queryModelCatalog(items, { compatibleWith: context });
  return (
    compatible.find((item) => item.value === current) ??
    compatible.find((item) => modelCompatibility(item, context) === 'full') ??
    compatible[0] ??
    null
  );
}
