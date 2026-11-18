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
/** Own one browsing session over a set of models: search text, folder, current tile and keys. */
import { computed, ref } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import { filterModelChoices } from '../ModelPickerChoices.js';
import { modelChoiceCompletion } from '../ModelPickerCompletion.js';
import { modelWallNeighbor } from '../ModelPickerNavigation.js';

/** Keys that move the current tile across the wall. */
const WALL_KEYS = new Set(['ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Home', 'End']);

/**
 * Browse the given models the way every picker popup does.
 *
 * The attached field and the standalone chooser share this, so search, folders, completion and
 * wall movement cannot drift apart. It never commits a choice: the caller decides what choosing
 * the current model means.
 */
export function useModelBrowsing(items: () => readonly ModelCatalogItem[]) {
  const query = ref('');
  const folder = ref('');
  const active = ref(0);
  // The wall reports its measured width; until then it lays out at its own initial width.
  const wallWidth = ref(540);
  const visible = computed(() => filterModelChoices(items(), query.value, folder.value));
  const navigable = computed(() => filterModelChoices(items(), query.value, ''));
  const completion = computed(() =>
    modelChoiceCompletion(visible.value[active.value], query.value),
  );
  return {
    query,
    folder,
    active,
    wallWidth,
    /** Models matching the search inside the open folder, in wall order. */
    visible,
    /** Models matching the search in every folder, from which the folder strip is built. */
    navigable,
    /** The display-only completion of the search text against the current model. */
    completion,
    /** Start at the root with no search, on the model with this value when it is shown. */
    restart(current = ''): void {
      query.value = '';
      folder.value = '';
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === current),
      );
    },
    /** Replace the search text and return to the first match. */
    search(text: string): void {
      query.value = text;
      active.value = 0;
    },
    /** Open one folder and return to its first model. */
    open(path: string): void {
      folder.value = path;
      active.value = 0;
    },
    /** Extend the search to its completion, keeping the same model current. */
    complete(): boolean {
      const accepted = completion.value;
      if (!accepted) return false;
      const highlighted = visible.value[active.value]?.value;
      query.value = accepted.text;
      active.value = Math.max(
        0,
        visible.value.findIndex((item) => item.value === highlighted),
      );
      return true;
    },
    /** Move the current tile for one wall key; other keys are not handled. */
    move(key: string): boolean {
      if (!WALL_KEYS.has(key)) return false;
      active.value = modelWallNeighbor(visible.value, wallWidth.value, active.value, key);
      return true;
    },
  };
}
