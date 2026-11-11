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
/** Resolve the active checkpoint's family using prepared catalog and native field boundaries. */
import type { ModelCatalogSnapshot } from '../../model_library/ModelCatalog.js';
import type { NativeEditorCard } from './NativeEditorCardAdapter.js';
import type { PresetAssociation } from '../domain/UserPreset.js';
import { modelFamilyScopes } from '../domain/ModelFamilyScopes.js';
import { recognizeModelField } from '../../model_library/ModelFieldRecognition.js';
import type { ModelCatalogItem } from '../../model_library/ModelCatalog.js';

/** Share the first projection-ordered generative model across all preset consumers of one panel. */
export function editorModelScopes(
  card: NativeEditorCard,
  cards: readonly NativeEditorCard[],
  snapshot: ModelCatalogSnapshot | null,
): PresetAssociation[] {
  if (!snapshot) return [];
  const panel = card.root.closest('.sugarcubes-workflow-masonry, .sugarcubes-cube-face');
  const candidates = (panel ? cards.filter((candidate) => panel.contains(candidate.root)) : [card])
    .filter((candidate) => candidate.root.isConnected)
    .sort(projectionOrder);
  for (const candidate of candidates)
    for (const field of [...candidate.modelInputs].sort((a, b) => a.name.localeCompare(b.name))) {
      const kind = recognizeModelField(
        snapshot,
        candidate.nodeType,
        field.name,
        field.options(),
        field.origins,
      );
      if (kind !== 'checkpoints' && kind !== 'diffusion_models') continue;
      const value = field.read();
      if (typeof value !== 'string' || !value.trim()) continue;
      const item = matchingModel(
        value,
        snapshot.items.filter((item) => item.kind === kind),
      );
      return modelFamilyScopes(
        item
          ? {
              baseModel: item.baseModel,
              architecture: item.architecture?.base ?? '',
              title: item.title,
              tags: item.tags ?? [],
            }
          : null,
      );
    }
  return [];
}

/** Follow authored projection order even when retained DOM mounts were created in another order. */
function projectionOrder(left: NativeEditorCard, right: NativeEditorCard): number {
  const order = (card: NativeEditorCard): number => {
    const value = card.root.closest<HTMLElement>('[data-sc-projection-order]')?.dataset
      .scProjectionOrder;
    return value !== undefined && /^\d+$/.test(value) ? Number(value) : Number.MAX_SAFE_INTEGER;
  };
  return (
    order(left) - order(right) ||
    (left.root.compareDocumentPosition(right.root) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1)
  );
}

/** Match Substitute's normalized backend-path and filename-stem lookup without inferring families from paths. */
function matchingModel(value: string, items: readonly ModelCatalogItem[]): ModelCatalogItem | null {
  const normalize = (value: string): string => value.trim().replaceAll('\\', '/').toLowerCase();
  const stem = (value: string): string =>
    normalize(value)
      .split('/')
      .at(-1)
      ?.replace(/\.[^.]+$/, '') ?? '';
  return (
    items.find((item) =>
      [item.value, item.relativePath].some((path) => normalize(path) === normalize(value)),
    ) ??
    items.find((item) =>
      [item.value, item.relativePath].some((path) => stem(path) === stem(value)),
    ) ??
    null
  );
}
