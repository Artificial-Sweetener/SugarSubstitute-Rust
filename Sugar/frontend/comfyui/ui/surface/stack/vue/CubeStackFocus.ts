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
/** Retain the same native focused control across keyed rail and cart permutations. */
import { onBeforeUpdate, onUpdated, type Ref } from 'vue';

/** Restore focus only when a Vue move temporarily leaves the original live control unfocused. */
export function useCubeStackFocus(root: Ref<HTMLElement | null>): void {
  let focused: HTMLElement | null = null;
  onBeforeUpdate(() => {
    const active = root.value?.ownerDocument.activeElement;
    focused = active instanceof HTMLElement && root.value?.contains(active) ? active : null;
  });
  onUpdated(() => {
    if (focused?.isConnected && focused.ownerDocument.activeElement === focused.ownerDocument.body)
      focused.focus({ preventScroll: true });
    focused = null;
  });
}
