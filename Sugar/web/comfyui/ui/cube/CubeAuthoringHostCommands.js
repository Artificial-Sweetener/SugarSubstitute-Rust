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
import { isCubeNode } from './node/ComfyCubeNodeFactory.js';
/** Own the host-visible entry points for creating and promoting native Cubes. */
export class CubeAuthoringHostCommands {
    #createCubeFromSelection;
    #createCubeFromSubgraph;
    #createEmptyCube;
    #canAuthor;
    /** Bind the application operations exposed through Comfy's canvas context menu. */
    constructor(options) {
        this.#canAuthor = options.canAuthor;
        this.#createCubeFromSelection = options.createCubeFromSelection;
        this.#createCubeFromSubgraph = options.createCubeFromSubgraph;
        this.#createEmptyCube = options.createEmptyCube;
    }
    /** Build graph-authoring actions for Comfy's native canvas context menu. */
    getCanvasMenuItems(canvas) {
        if (!this.#canAuthor())
            return [];
        const selectedItems = readSelectedItems(canvas);
        const selectedCount = selectedItems.length;
        const options = [
            {
                content: 'Create Empty SugarCube',
                callback: this.#createEmptyCube,
            },
        ];
        if (selectedCount > 0 && !selectedItems.some(isCubeNode)) {
            options.push({
                content: 'Create SugarCube from Selection',
                callback: this.#createCubeFromSelection,
            });
        }
        if (selectedCount === 1 && isOrdinarySubgraphNode(selectedItems[0])) {
            options.push({
                content: 'Convert Selected Subgraph to SugarCube',
                callback: this.#createCubeFromSubgraph,
            });
        }
        return options;
    }
}
/** Read only the selected-item count exposed by Comfy's untyped canvas host. */
function readSelectedItems(canvas) {
    if (!canvas || typeof canvas !== 'object')
        return [];
    const selectedItems = Reflect.get(canvas, 'selectedItems');
    if (!selectedItems || typeof selectedItems !== 'object')
        return [];
    const iterator = Reflect.get(selectedItems, Symbol.iterator);
    return typeof iterator === 'function' ? Array.from(selectedItems) : [];
}
/** Accept one native Subgraph only when it has not already become a Cube. */
function isOrdinarySubgraphNode(value) {
    if (isCubeNode(value) || !value || typeof value !== 'object')
        return false;
    const isSubgraphNode = Reflect.get(value, 'isSubgraphNode');
    return typeof isSubgraphNode === 'function' && isSubgraphNode.call(value) === true;
}
