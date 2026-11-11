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
/** Adapt final LiteGraph menu production for marked Cube operands. */

import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';

interface NativeMenuCanvas {
  selectedItems: Set<unknown>;
  getNodeMenuOptions(node: unknown): unknown[];
  getCanvasMenuOptions(): unknown[];
}

const CUBE_NODE_MENU_ACTIONS = new Set([
  'Convert to Subgraph',
  'Edit Subgraph Widgets',
  'Unpack Subgraph',
  'Node Info',
]);

/** Filter native structural and Subgraph-authoring items for Cube operands. */
export class ComfyCubeNativeMenuAdapter {
  readonly #canvas: NativeMenuCanvas;
  readonly #nativeNodeMenu: NativeMenuCanvas['getNodeMenuOptions'];
  readonly #nativeCanvasMenu: NativeMenuCanvas['getCanvasMenuOptions'];
  #installed = false;

  /** Validate the stable LiteGraph final-menu methods used by both menu renderers. */
  constructor(canvas: unknown) {
    if (
      !isRecord(canvas) ||
      !(canvas.selectedItems instanceof Set) ||
      typeof canvas.getNodeMenuOptions !== 'function' ||
      typeof canvas.getCanvasMenuOptions !== 'function'
    ) {
      throw new TypeError('Comfy native menu integration is unavailable.');
    }
    this.#canvas = canvas as unknown as NativeMenuCanvas;
    this.#nativeNodeMenu = this.#canvas.getNodeMenuOptions;
    this.#nativeCanvasMenu = this.#canvas.getCanvasMenuOptions;
  }

  /** Install instance-scoped final-menu filters without changing LiteGraph prototypes. */
  install(): void {
    if (this.#installed) return;
    this.#canvas.getNodeMenuOptions = (node: unknown) => {
      const options = this.#nativeNodeMenu.call(this.#canvas, node);
      if (!isCubeNode(node)) return options;
      return filterMenuItems(options, CUBE_NODE_MENU_ACTIONS);
    };
    this.#canvas.getCanvasMenuOptions = () => {
      const options = this.#nativeCanvasMenu.call(this.#canvas);
      return [...this.#canvas.selectedItems].some(isCubeNode)
        ? filterMenuItems(options, new Set(['Convert to Subgraph']))
        : options;
    };
    this.#installed = true;
  }

  /** Restore host-owned menu methods when the integration is disposed. */
  dispose(): void {
    if (!this.#installed) return;
    this.#canvas.getNodeMenuOptions = this.#nativeNodeMenu;
    this.#canvas.getCanvasMenuOptions = this.#nativeCanvasMenu;
    this.#installed = false;
  }
}

/** Filter only hard-coded pre-translation LiteGraph source labels in one compatibility seam. */
function filterMenuItems(options: readonly unknown[], blocked: ReadonlySet<string>): unknown[] {
  return options.filter(
    (value) => !isRecord(value) || typeof value.content !== 'string' || !blocked.has(value.content),
  );
}
