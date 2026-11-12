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
/** One pointer-target adapter admits only a genuine capture's prepared semantic IDs and actual native-value membership. */
import type { NativeReorderCapture } from './reorder-command.js';
import { nativeReorderRecipe, nativeReorderElement } from './reorder-native-recipe.js';
/** Measured canonical geometry takes precedence; the native DOM fallback must belong to the same actual selected source recipe. */
export function readReorderPointerTarget(
  capture: NativeReorderCapture,
  event: Pick<PointerEvent, 'target'>,
  visualHit: () => number | null,
): number | null {
  const recipe = nativeReorderRecipe(capture.context.values, capture.target.entry),
    node =
      event.target instanceof Element
        ? event.target.closest<HTMLElement>('[data-reorder-chip]')
        : null,
    id =
      visualHit() ??
      Number(
        node &&
          (!node.hasAttribute('data-reorder-native-value') ||
            (recipe && nativeReorderElement(node, recipe.key)))
          ? node.dataset.reorderChip
          : NaN,
      );
  return Number.isSafeInteger(id) && capture.prepared.chips[id] ? id : null;
}
