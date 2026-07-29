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
/** Adapt Cube authoring to Comfy's command contract. */

/** Describe one declarative Comfy canvas context-menu item. */
export interface CubeAuthoringCanvasMenuItem {
  content: string;
  callback?: () => unknown;
}

/** Own the host-visible entry points for creating and promoting native Cubes. */
export class CubeAuthoringHostCommands {
  readonly #createCubeFromSelection: () => unknown;
  readonly #createCubeFromSubgraph: () => unknown;
  readonly #createEmptyCube: () => unknown;

  /** Bind the application operations exposed through Comfy's canvas context menu. */
  constructor(
    createCubeFromSelection: () => unknown,
    createCubeFromSubgraph: () => unknown,
    createEmptyCube: () => unknown,
  ) {
    this.#createCubeFromSelection = createCubeFromSelection;
    this.#createCubeFromSubgraph = createCubeFromSubgraph;
    this.#createEmptyCube = createEmptyCube;
  }

  /** Build graph-authoring actions for Comfy's native canvas context menu. */
  getCanvasMenuItems(canvas: unknown): CubeAuthoringCanvasMenuItem[] {
    const selectedCount = readSelectedItemCount(canvas);
    const options: CubeAuthoringCanvasMenuItem[] = [
      {
        content: 'Create Empty SugarCube',
        callback: this.#createEmptyCube,
      },
    ];
    if (selectedCount > 0) {
      options.push({
        content: 'Create SugarCube from Selection',
        callback: this.#createCubeFromSelection,
      });
    }
    if (selectedCount === 1) {
      options.push({
        content: 'Convert Selected Subgraph to SugarCube',
        callback: this.#createCubeFromSubgraph,
      });
    }
    return options;
  }
}

/** Read only the selected-item count exposed by Comfy's untyped canvas host. */
function readSelectedItemCount(canvas: unknown): number {
  if (!canvas || typeof canvas !== 'object') return 0;
  const selectedItems = Reflect.get(canvas, 'selectedItems');
  if (!selectedItems || typeof selectedItems !== 'object') return 0;
  const count = Number(Reflect.get(selectedItems, 'size'));
  return Number.isFinite(count) && count > 0 ? count : 0;
}
