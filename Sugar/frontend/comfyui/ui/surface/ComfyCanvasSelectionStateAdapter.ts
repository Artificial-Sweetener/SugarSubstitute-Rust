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
/** Adapt Comfy's root selection to Cube editor navigation state. */

/** Own capture and restoration of Comfy's authoritative selection set. */
export class ComfyCanvasSelectionStateAdapter {
  readonly #selectedItems: Set<unknown>;
  readonly #updateSelectedItems: (() => void) | null;
  readonly #logger: Pick<Console, 'debug'> | null;

  /** Bind the live Comfy selection and its derived-state refresh. */
  constructor(
    selectedItems: Set<unknown>,
    updateSelectedItems: (() => void) | null,
    logger: Pick<Console, 'debug'> | null = null,
  ) {
    this.#selectedItems = selectedItems;
    this.#updateSelectedItems = updateSelectedItems;
    this.#logger = logger;
  }

  /** Capture the exact root-graph item objects Comfy currently selected. */
  capture(): readonly unknown[] {
    const items = [...this.#selectedItems];
    this.#logger?.debug(`SugarCubes captured ${String(items.length)} selected surface item(s).`);
    return items;
  }

  /** Restore one root selection after Comfy returns from another graph. */
  restore(items: readonly unknown[]): void {
    this.#selectedItems.clear();
    for (const item of items) this.#selectedItems.add(item);
    this.#updateSelectedItems?.();
    this.#logger?.debug(`SugarCubes restored ${String(items.length)} selected surface item(s).`);
  }
}
