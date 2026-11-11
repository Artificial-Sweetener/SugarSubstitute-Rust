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
/** Position a newly placed Cube directly after one existing Cube. */
/** Preserve a visible proximity-link corridor between neighboring Cubes. */
export const MINIMUM_CUBE_CHAIN_GAP = 24;
/** Own width-aware insert-after geometry for the ordered Cube chain. */
export class CubeNodeInsertAfterCoordinator {
    #nodes;
    /** Bind the authoritative live Cube inventory. */
    constructor(nodes) {
        this.#nodes = nodes;
    }
    /** Return the initial insertion origin without mutating graph-owned nodes. */
    insertionOrigin(source) {
        const neighbor = this.#downstream(source, null)[0];
        return [rightEdge(source) + insertionGap(source, neighbor), readPosition(source)[1]];
    }
    /** Insert one graph-owned Cube and shift every downstream Cube without creating links. */
    insertAfter(source, inserted) {
        const downstream = this.#downstream(source, inserted);
        const neighbor = downstream[0];
        const gap = insertionGap(source, neighbor);
        const insertionX = rightEdge(source) + gap;
        writePosition(inserted, [insertionX, readPosition(source)[1]]);
        if (neighbor) {
            const shift = insertionX + readWidth(inserted) + gap - readPosition(neighbor)[0];
            for (const node of downstream) {
                const position = readPosition(node);
                writePosition(node, [position[0] + Math.max(0, shift), position[1]]);
                this.#nodes.changed(node);
            }
        }
        this.#nodes.changed(inserted);
    }
    /** Resolve existing Cubes after the source using the same stable order as swap actions. */
    #downstream(source, inserted) {
        const ordered = this.#nodes
            .list()
            .filter((node) => node !== inserted)
            .sort(compareCubeNodePosition);
        const index = ordered.indexOf(source);
        return index < 0 ? [] : ordered.slice(index + 1);
    }
}
/** Preserve an authored gap when it is already safe for proximity matching. */
function insertionGap(source, neighbor) {
    if (!neighbor)
        return MINIMUM_CUBE_CHAIN_GAP;
    const authored = readPosition(neighbor)[0] - rightEdge(source);
    return Math.max(MINIMUM_CUBE_CHAIN_GAP, authored);
}
/** Sort left-to-right with a vertical tie-breaker for stable chain behavior. */
function compareCubeNodePosition(left, right) {
    const leftPosition = readPosition(left);
    const rightPosition = readPosition(right);
    return leftPosition[0] - rightPosition[0] || leftPosition[1] - rightPosition[1];
}
/** Read one live right edge from finite host geometry. */
function rightEdge(node) {
    return readPosition(node)[0] + readWidth(node);
}
/** Read one finite non-negative Cube width. */
function readWidth(node) {
    return Math.max(0, finite(node.size[0]));
}
/** Read one Cube position as finite graph-space coordinates. */
function readPosition(node) {
    return [finite(node.pos[0]), finite(node.pos[1])];
}
/** Write a Cube position through the native API when available. */
function writePosition(node, position) {
    node.setPos?.(position[0], position[1]);
    node.pos[0] = position[0];
    node.pos[1] = position[1];
}
/** Prevent invalid persisted geometry from entering placement math. */
function finite(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number : 0;
}
