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
/** Observe selected catalog identities without tying daily freshness to a rendered field. */
import { watch } from 'vue';
import type { ModelCatalogItem } from '../ModelCatalog.js';
import type { ModelUpdatePort } from '../ModelUpdates.js';
import type { ModelPickerFeedback } from '../ModelPickerActions.js';

/** Check selected diffusion models and LoRAs while the backend owns timing and workflow usage. */
export function useModelSelectionUpdates(
  selected: () => ModelCatalogItem | undefined,
  updates: ModelUpdatePort | null,
  feedback: ModelPickerFeedback | null,
): void {
  watch(
    selected,
    (item) => {
      if (
        !item ||
        !['checkpoints', 'diffusion_models', 'loras'].includes(item.kind) ||
        !/^[a-f0-9]{64}$/.test(item.id)
      )
        return;
      void updates?.checkSelection(item.id).catch((error: unknown) => {
        feedback?.(
          'error',
          error instanceof Error ? error.message : 'Model updates could not be checked.',
        );
      });
    },
    { immediate: true },
  );
}
