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
/** Apply title-editing policy before Comfy's native header double-click handler. */
export class NativeProjectedNodeTitleBoundary {
  readonly #target: HTMLElement;

  /** Bind once to the stable mount so native header replacement retains the policy. */
  constructor(target: HTMLElement, allowRename: boolean) {
    this.#target = target;
    if (!allowRename) target.addEventListener('dblclick', this.#blockRename, true);
  }

  /** Release the owned listener when the projected card unmounts. */
  dispose(): void {
    this.#target.removeEventListener('dblclick', this.#blockRename, true);
  }

  /** Shield title rows while preserving native widget double-click and header click actions. */
  readonly #blockRename = (event: Event): void => {
    const header = event.target instanceof Element ? event.target.closest('.lg-node-header') : null;
    if (header && this.#target.contains(header)) event.stopPropagation();
  };
}
