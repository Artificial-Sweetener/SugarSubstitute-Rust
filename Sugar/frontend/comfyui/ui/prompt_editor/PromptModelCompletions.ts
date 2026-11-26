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
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import type {
  ModelCompletionPopup,
  ModelCompletionPopupHost,
} from '../../../sugarprompt/editor/src/model-completion-popup.js';
import type { LoraCatalogItem } from '../../../sugarprompt/services/src/index.js';
import type { ModelCatalogItem } from '../model_library/ModelCatalog.js';
import type { ModelGallerySurface } from '../model_library/ModelPickerHost.js';

/** The model library capability that mounts its picker gallery for another owner's candidates. */
export interface PromptModelGallery {
  mountGallery(events: { activate(index: number): void }): ModelGallerySurface;
  /** Let a person choose one installed model of a kind from the picker popup at a point. */
  chooseModel(
    kind: string,
    label: string,
    anchor: { left: number; top: number; width: number; height: number },
  ): Promise<ModelCatalogItem | null>;
  /** The rows about one model, the same ones every model picker surface shows. */
  modelMenu(item: ModelCatalogItem): readonly ContextMenuEntry[];
}

/**
 * Adapt the model library's gallery to the editor's candidate popup.
 *
 * The editor ranks LoRAs by their installed identity; `model` resolves each identity to the
 * catalog record the gallery draws. A candidate the catalog no longer holds is left out, and the
 * positions the editor and the gallery speak in are translated both ways.
 */
export function promptModelCompletions(
  gallery: Pick<PromptModelGallery, 'mountGallery'>,
  model: (backendValue: string) => ModelCatalogItem | null,
  watch: (changed: () => void) => () => void,
): ModelCompletionPopupHost {
  return {
    mount(events): ModelCompletionPopup {
      // Gallery position -> candidate position, for the candidates currently shown.
      let candidates: readonly number[] = [];
      // What the editor last asked to show, so a changed catalog can be drawn again.
      let asked: { readonly models: readonly LoraCatalogItem[]; readonly selected: number } | null =
        null;
      const surface = gallery.mountGallery({
        activate: (index) => {
          const candidate = candidates[index];
          if (candidate !== undefined) events.activate(candidate);
        },
      });
      const show = (models: readonly LoraCatalogItem[], selected: number): void => {
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
      };
      // The popup follows the catalog while it is open: an image that arrives shows on its card.
      const unwatch = watch(() => {
        if (asked) show(asked.models, asked.selected);
      });
      return {
        element: surface.element,
        update(models, selected) {
          asked = { models, selected };
          show(models, selected);
        },
        neighbor(selected, key) {
          const active = candidates.indexOf(selected);
          if (active < 0) return selected;
          return candidates[surface.neighbor(active, key)] ?? selected;
        },
        dispose: () => {
          unwatch();
          surface.dispose();
        },
      };
    },
  };
}
