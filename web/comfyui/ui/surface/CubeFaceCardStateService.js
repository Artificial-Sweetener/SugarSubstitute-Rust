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
/** Persist shared card visibility and activation through native workflow history. */
import { isCubeNode, requireCubeSurface, } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';
import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { setCubeFaceCardRevealed, setCubeFaceNodeEnabled } from './CubeFaceCardStateController.js';
import { parseCubeSurfaceState, serializeCubeSurfaceState, } from './CubeSurfaceState.js';
import { replaceCubeSurfaceRecord } from './CubeSurfaceRecordHelpers.js';
/** Read the same presentation schema from a Cube or loose-node workflow without mutating it. */
export function readCubeFaceCardState(target) {
    return parseCubeSurfaceState(isCubeNode(target)
        ? target.properties.sugarcubes_surface
        : isRecord(target.extra)
            ? target.extra.sugarcubes_surface
            : undefined);
}
/** Own persisted presentation changes for both Cube faces and workflow sections. */
export class CubeFaceCardStateService {
    #nodes;
    #history;
    #listeners = new Set();
    /** Bind shared card transitions to workflow history and Cube invalidation. */
    constructor(options) {
        this.#nodes = options.nodes;
        this.#history = options.history;
    }
    /** Observe changes from either native Cube faces or loose-node presentation controls. */
    subscribe(listener) {
        const unsubscribe = this.#nodes.subscribe(listener);
        this.#listeners.add(listener);
        return () => {
            unsubscribe();
            this.#listeners.delete(listener);
        };
    }
    /** Return optional-card choices from the authoritative renderer-neutral policy. */
    list(target) {
        const graph = isCubeNode(target) ? target.subgraph : target;
        return resolveCubeFaceCardPresentation(cardNodes(target), readCubeFaceCardState(target), graph)
            .menuEntries;
    }
    /** Persist a reveal choice while preserving its independent activation choice. */
    setRevealed(target, nodeId, revealed) {
        this.#change(target, nodeId, (state, node) => setCubeFaceCardRevealed(state, node, revealed));
    }
    /** Persist activation without changing whether the card is optional or revealed. */
    setEnabled(target, nodeId, enabled) {
        this.#change(target, nodeId, (state, node) => setCubeFaceNodeEnabled(state, node, enabled));
    }
    /** Apply one shared policy transition and notify the presentation after persistence. */
    #change(target, nodeId, change) {
        const node = cardNodes(target).find((candidate) => String(candidate.id ?? '') === nodeId);
        if (!node)
            return;
        const state = readCubeFaceCardState(target);
        this.#history.beforeChange?.();
        change(state, node);
        const serialized = serializeCubeSurfaceState(state);
        if (isCubeNode(target)) {
            replaceCubeSurfaceRecord(requireCubeSurface(target), serialized);
            this.#nodes.changed(target);
        }
        else {
            const extra = isRecord(target.extra) ? target.extra : {};
            target.extra = extra;
            extra.sugarcubes_surface = serialized;
            for (const listener of this.#listeners)
                listener();
        }
        this.#history.setDirtyCanvas?.(true, true);
        this.#history.afterChange?.();
    }
}
/** Exclude Cube wrappers from the workflow's loose-card state. */
function cardNodes(target) {
    return isCubeNode(target)
        ? target.subgraph._nodes
        : (target._nodes ?? target.nodes ?? []).filter((node) => !isCubeNode(node));
}
