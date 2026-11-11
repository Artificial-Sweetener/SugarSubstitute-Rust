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
/** Own cancellable feedback for each mounted model-field action lifetime. */
import { onBeforeUnmount, ref } from 'vue';

/** Keep progress, terminal feedback, and cancellation within one presentation lifetime. */
export function useModelAction(disabled: () => boolean = () => false) {
  const busy = ref(false);
  const status = ref('');
  const error = ref('');
  let controller: AbortController | null = null;
  onBeforeUnmount(() => controller?.abort());

  /** Finish progress on failure while retaining deliberate cancellation feedback. */
  const run = async (operation: (signal: AbortSignal) => Promise<void>): Promise<void> => {
    if (busy.value || disabled()) return;
    const current = new AbortController();
    controller = current;
    busy.value = true;
    status.value = '';
    error.value = '';
    try {
      await operation(current.signal);
    } catch (failure: unknown) {
      if (!current.signal.aborted) {
        status.value = '';
        error.value = failure instanceof Error ? failure.message : 'The model action failed.';
        console.error('SugarCubes model action failed.', failure);
      }
    } finally {
      busy.value = false;
      controller = null;
    }
  };

  /** Cancel work through its owner and leave the terminal message visible. */
  const cancel = (): void => {
    controller?.abort();
    status.value = 'Cancelled.';
  };

  return { busy, status, error, run, cancel };
}
