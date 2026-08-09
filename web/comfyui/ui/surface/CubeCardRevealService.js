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
/** Own graph-persisted Cube card reveal transitions outside renderer chrome. */
import { requireCubeSurface } from '../cube/node/ComfyCubeNodeFactory.js';
import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { setCubeFaceCardRevealed } from './CubeFaceCardStateController.js';
import { parseCubeSurfaceState, serializeCubeSurfaceState } from './CubeSurfaceState.js';
/** Persist reveal changes once, then notify both Nodes renderers through the catalog. */
export class CubeCardRevealService {
    #nodes;
    #history;
    /** Bind state transitions to graph history and Cube presentation invalidation. */
    constructor(options) {
        this.#nodes = options.nodes;
        this.#history = options.history;
    }
    /** Return current optional-card choices from the shared renderer-neutral policy. */
    list(node) {
        const state = parseCubeSurfaceState(requireCubeSurface(node));
        return resolveCubeFaceCardPresentation(node.subgraph._nodes, state, node.subgraph).menuEntries;
    }
    /** Persist one optional-card reveal choice through native graph history. */
    setRevealed(node, nodeId, revealed) {
        const internalNode = node.subgraph._nodes.find((candidate) => String(candidate.id ?? '') === nodeId);
        if (!internalNode)
            return;
        const surface = requireCubeSurface(node);
        const state = parseCubeSurfaceState(surface);
        this.#history.beforeChange?.();
        setCubeFaceCardRevealed(state, internalNode, revealed);
        replaceRecord(surface, serializeCubeSurfaceState(state));
        this.#nodes.changed(node);
        this.#history.setDirtyCanvas?.(true, true);
        this.#history.afterChange?.();
    }
}
/** Replace one persisted state record without changing its host-owned identity. */
function replaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
