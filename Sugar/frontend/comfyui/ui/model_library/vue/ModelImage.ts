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
/** Bind one component to the model image presentation every model surface shares. */
import { onBeforeUnmount, shallowRef, watch } from 'vue';
import { ModelImagePresentation } from '../ModelImagePresentation.js';

export interface ShownModelImage {
  /** The address to draw, or null. */
  readonly url: string | null;
  /** Whether the first image is still being decoded. */
  readonly loading: boolean;
  /** Whether there is no image to draw. */
  readonly unavailable: boolean;
}

/**
 * Follow an image address through the shared presentation. `record` names what the address came
 * from, so a failed address is tried again when that changes; it defaults to the address.
 */
export function useModelImage(
  url: () => string | null,
  record: () => unknown = url,
): () => ShownModelImage {
  const drawn = shallowRef(0);
  const presentation = new ModelImagePresentation(() => {
    drawn.value++;
  });
  watch([url, record], ([wanted, from]) => presentation.show(wanted, from), {
    immediate: true,
    flush: 'sync',
  });
  onBeforeUnmount(() => presentation.dispose());
  return () => {
    // Reading the counter makes a render depend on every change the presentation reports.
    void drawn.value;
    return {
      url: presentation.shown,
      loading: presentation.loading,
      unavailable: presentation.unavailable,
    };
  };
}
