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
/** Present the prompt editor's LoRA candidates in the model library's own picker gallery. */
import type {
  ModelCompletionPopup,
  ModelCompletionPopupHost,
} from '../../../sugarprompt/editor/src/model-completion-popup.js';
import type { ModelCatalogItem } from '../model_library/ModelCatalog.js';
import type { ModelGallerySurface } from '../model_library/ModelPickerHost.js';

/** The model library capability that mounts its picker gallery for another owner's candidates. */
export interface PromptModelGallery {
  mountGallery(events: { activate(index: number): void }): ModelGallerySurface;
}

/**
 * Adapt the model library's gallery to the editor's candidate popup.
 *
 * The editor ranks LoRAs by their installed identity; `model` resolves each identity to the
 * catalog record the gallery draws. A candidate the catalog no longer holds is left out, and the
 * positions the editor and the gallery speak in are translated both ways.
 */
export function promptModelCompletions(
  gallery: PromptModelGallery,
  model: (backendValue: string) => ModelCatalogItem | null,
): ModelCompletionPopupHost {
  return {
    mount(events): ModelCompletionPopup {
      // Gallery position -> candidate position, for the candidates currently shown.
      let candidates: readonly number[] = [];
      const surface = gallery.mountGallery({
        activate: (index) => {
          const candidate = candidates[index];
          if (candidate !== undefined) events.activate(candidate);
        },
      });
      return {
        element: surface.element,
        update(models, selected) {
          const items: ModelCatalogItem[] = [];
          const shown: number[] = [];
          for (const [index, candidate] of models.entries()) {
            const item = model(candidate.model.backendValue);
            if (!item) continue;
            items.push(item);
            shown.push(index);
          }
          candidates = shown;
          surface.update(items, Math.max(0, shown.indexOf(selected)));
        },
        neighbor(selected, key) {
          const active = candidates.indexOf(selected);
          if (active < 0) return selected;
          return candidates[surface.neighbor(active, key)] ?? selected;
        },
        dispose: () => surface.dispose(),
      };
    },
  };
}
