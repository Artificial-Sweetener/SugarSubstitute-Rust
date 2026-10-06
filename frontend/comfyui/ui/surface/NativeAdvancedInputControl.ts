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
/** Expose the exact projected node's advanced disclosure without borrowing a footer or graph ID. */
import type { ComfyNode } from '../types/graph.js';
import { cubeFaceUnconsumedWidgets } from './CubeFaceWidgetPolicy.js';

export interface NativeAdvancedInputControl {
  shown(): boolean;
  setShown(shown: boolean): void;
}

/** Retain advanced actions through collapse while leaving native state and history authoritative. */
export function nativeAdvancedInputControl(
  node: ComfyNode,
  refresh: () => void,
): NativeAdvancedInputControl | null {
  if (
    !cubeFaceUnconsumedWidgets(node).some(
      (widget) => widget.options?.advanced === true || widget.advanced === true,
    )
  )
    return null;
  return {
    shown: () => node.showAdvanced === true,
    setShown: (shown) => {
      if (shown === (node.showAdvanced === true)) return;
      node.graph?.beforeChange?.();
      node.showAdvanced = shown;
      node.graph?.afterChange?.();
      refresh();
    },
  };
}
