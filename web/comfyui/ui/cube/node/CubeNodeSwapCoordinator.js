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
/** Swap first-class Cube nodes without depending on legacy group chrome state. */
import { resolveCubeNodeSwapPlacements } from './CubeNodeSwapGeometry.js';
const MINIMUM_SWAP_GAP = 24;
/** Own node-based Cube swap eligibility and state transitions. */
export class CubeNodeSwapCoordinator {
    #graph;
    #nodes;
    #history;
    #setDirtyCanvas;
    /** Bind graph-owned Cube nodes and the native history seam. */
    constructor(options) {
        this.#graph = options.graph;
        this.#nodes = options.nodes;
        this.#history = options.history;
        this.#setDirtyCanvas = options.setDirtyCanvas ?? (() => undefined);
    }
    /** Return whether a neighboring first-class Cube node can be swapped in that direction. */
    canSwap(metadata, direction) {
        return this.#resolvePlan(metadata, direction) !== null;
    }
    /** Swap this Cube node with its nearest eligible first-class Cube neighbor. */
    swap(metadata, direction) {
        const plan = this.#resolvePlan(metadata, direction);
        if (!plan)
            return;
        const formerLeft = direction === 'left' ? plan.neighbor : plan.current;
        const formerRight = direction === 'left' ? plan.current : plan.neighbor;
        const placements = resolveCubeNodeSwapPlacements(readSwapBounds(formerLeft), readSwapBounds(formerRight), MINIMUM_SWAP_GAP);
        this.#history.beforeChange?.();
        writePosition(formerLeft, placements.formerLeft);
        writePosition(formerRight, placements.formerRight);
        this.#graph.afterChange?.();
        this.#graph.setDirtyCanvas?.(true, true);
        this.#setDirtyCanvas(true, true);
        this.#nodes.changed(plan.current);
        this.#nodes.changed(plan.neighbor);
        this.#history.afterChange?.();
    }
    /** Resolve the nearest eligible Cube node using current node positions. */
    #resolvePlan(metadata, direction) {
        const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
        if (!instanceId)
            return null;
        const current = this.#nodes.get(instanceId);
        if (!current)
            return null;
        const ordered = [...this.#nodes.list()].sort(compareCubeNodePosition);
        const index = ordered.indexOf(current);
        if (index < 0)
            return null;
        const neighbor = ordered[index + (direction === 'left' ? -1 : 1)] ?? null;
        return neighbor && hasReorderableInput(current) && hasReorderableInput(neighbor)
            ? { current, neighbor }
            : null;
    }
}
/** Keep source Cubes fixed at the start of a series. */
function hasReorderableInput(node) {
    return node.inputs.length > 0;
}
/** Sort left-to-right with a vertical tie-breaker for stable row-independent swaps. */
function compareCubeNodePosition(left, right) {
    const leftPosition = readPosition(left);
    const rightPosition = readPosition(right);
    return leftPosition[0] - rightPosition[0] || leftPosition[1] - rightPosition[1];
}
/** Read a native Cube node position as finite graph-space coordinates. */
function readPosition(node) {
    return [finite(node.pos[0]), finite(node.pos[1])];
}
/** Read the live node width at the moment a swap is executed. */
function readSwapBounds(node) {
    return {
        position: readPosition(node),
        width: Math.max(0, finite(node.size[0])),
    };
}
/** Write a native Cube node position through its native API when present. */
function writePosition(node, position) {
    if (typeof node.setPos === 'function') {
        node.setPos(position[0], position[1]);
    }
    node.pos[0] = position[0];
    node.pos[1] = position[1];
}
/** Prevent invalid persisted graph coordinates from entering swap math. */
function finite(value) {
    const numberValue = Number(value);
    return Number.isFinite(numberValue) ? numberValue : 0;
}
