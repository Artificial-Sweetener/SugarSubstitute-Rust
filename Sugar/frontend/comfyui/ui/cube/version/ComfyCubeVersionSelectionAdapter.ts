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
/** Preserve Comfy's native selection lifecycle across a Cube node replacement. */

import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import type { CubeReplacementSelection } from './ComfyCubeVersionReplacementAdapter.js';

/** Delegate replacement selection to Comfy so every renderer-owned selection index stays aligned. */
export class ComfyCubeVersionSelectionAdapter implements CubeReplacementSelection {
  readonly #selectedItems: Set<unknown>;
  readonly #processSelect: (node: CubeNode) => void;

  /** Bind Comfy's authoritative selection operation and its live selection set. */
  constructor(options: { selectedItems: Set<unknown>; processSelect(node: CubeNode): void }) {
    this.#selectedItems = options.selectedItems;
    this.#processSelect = options.processSelect;
  }

  /** Select the replacement through Comfy after removing stale object identities. */
  replace(source: CubeNode, target: CubeNode): void {
    this.#selectedItems.delete(source);
    this.#selectedItems.delete(target);
    this.#processSelect(target);
  }
}
