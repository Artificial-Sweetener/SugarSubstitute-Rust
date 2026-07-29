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
/** Translate legacy Cube marker layout into compact native subgraph boundaries. */
import { isRecord } from '../../types/common.js';
const BOUNDARY_GAP = 40;
const DEFAULT_BOUNDARY_WIDTH = 120;
const DEFAULT_BOUNDARY_HEIGHT = 40;
/** Keep empty-draft boundaries visible and clearly separated in the editor. */
export const EMPTY_CUBE_INPUT_POSITION = [120, 200];
/** Keep empty-draft boundaries visible and clearly separated in the editor. */
export const EMPTY_CUBE_OUTPUT_POSITION = [780, 200];
/** Frame both transient boundary affordances when an empty Cube editor first opens. */
export const EMPTY_CUBE_BOUNDARY_VIEWPORT_BOUNDS = [80, 120, 860, 260];
/** Name the definition-owned relation plan used to restore native boundaries. */
export const NATIVE_CUBE_BOUNDARY_LAYOUT_KEY = 'sugarcubes_boundary_layout';
/** Persist the internal node relations used to position native IO boundaries. */
export function attachNativeCubeBoundaryLayout(subgraph, inputNodes, outputNodes) {
    const plan = {
        schema: 1,
        inputSymbols: readNodeSymbols(inputNodes),
        outputSymbols: readNodeSymbols(outputNodes),
    };
    subgraph.extra = {
        ...(isRecord(subgraph.extra) ? subgraph.extra : {}),
        [NATIVE_CUBE_BOUNDARY_LAYOUT_KEY]: plan,
    };
}
/** Restore compact native boundaries beside the internal nodes they connect to. */
export function applyNativeCubeBoundaryLayout(subgraph) {
    const plan = readBoundaryPlan(subgraph.extra) ?? {
        schema: 1,
        inputSymbols: [],
        outputSymbols: [],
    };
    const nodesBySymbol = new Map();
    for (const node of subgraph._nodes) {
        const symbol = readNodeSymbol(node);
        if (symbol)
            nodesBySymbol.set(symbol, node);
    }
    const entries = [...nodesBySymbol.values()];
    if (!entries.length)
        return false;
    const inputChanged = writeConnectedBoundaryPosition(subgraph.inputNode, 'input', entries, resolveConnectedNodes(plan.inputSymbols, nodesBySymbol));
    const outputChanged = writeConnectedBoundaryPosition(subgraph.outputNode, 'output', entries, resolveConnectedNodes(plan.outputSymbols, nodesBySymbol));
    return inputChanged || outputChanged;
}
/** Place editable draft boundaries on opposite sides of an otherwise empty editor. */
export function applyEmptyCubeBoundaryLayout(subgraph) {
    const inputChanged = writeBoundaryPosition(subgraph.inputNode, EMPTY_CUBE_INPUT_POSITION);
    const outputChanged = writeBoundaryPosition(subgraph.outputNode, EMPTY_CUBE_OUTPUT_POSITION);
    return inputChanged || outputChanged;
}
/** Label Comfy's transient IO slots without creating persisted Cube boundaries. */
export function labelEmptyCubeBoundaryAffordances(subgraph) {
    if (subgraph.inputNode.emptySlot)
        subgraph.inputNode.emptySlot.name = 'Add Input';
    if (subgraph.outputNode.emptySlot)
        subgraph.outputNode.emptySlot.name = 'Add Output';
}
/** Resolve one persisted plan without trusting serialized graph extras. */
function readBoundaryPlan(extra) {
    if (!isRecord(extra))
        return null;
    const value = extra[NATIVE_CUBE_BOUNDARY_LAYOUT_KEY];
    if (!isRecord(value) || value.schema !== 1)
        return null;
    return {
        schema: 1,
        inputSymbols: readStringArray(value.inputSymbols),
        outputSymbols: readStringArray(value.outputSymbols),
    };
}
/** Return stable symbols for a relation-owned node set. */
function readNodeSymbols(nodes) {
    return [...nodes].map(readNodeSymbol).filter((symbol) => symbol !== null);
}
/** Read one node's stable Cube symbol. */
function readNodeSymbol(node) {
    const value = isRecord(node.properties) ? node.properties.sugarcubes_symbol : null;
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}
/** Resolve persisted symbols to the current graph-owned node objects. */
function resolveConnectedNodes(symbols, nodesBySymbol) {
    return symbols
        .map((symbol) => nodesBySymbol.get(symbol))
        .filter((node) => node !== undefined);
}
/** Place one boundary outside the graph and align it to its connected nodes. */
function writeBoundaryPosition(boundary, position) {
    let changed = false;
    const previousSlotGeometry = readBoundarySlotGeometry(boundary);
    if (boundary.boundingRect) {
        changed = writePosition(boundary.boundingRect, position) || changed;
    }
    if (boundary.bounding) {
        changed = writePosition(boundary.bounding, position) || changed;
    }
    if (boundary.pos) {
        changed = writePosition(boundary.pos, position) || changed;
    }
    boundary.arrange?.();
    return changed || previousSlotGeometry !== readBoundarySlotGeometry(boundary);
}
/** Snapshot slot anchors so boundary arrangement participates in change detection. */
function readBoundarySlotGeometry(boundary) {
    const slots = [...(boundary.slots ?? []), ...(boundary.emptySlot ? [boundary.emptySlot] : [])];
    return slots
        .map((slot) => {
        const pos = slot.pos;
        const bounds = slot.boundingRect;
        return [
            Number(pos?.[0]) || 0,
            Number(pos?.[1]) || 0,
            Number(bounds?.[0]) || 0,
            Number(bounds?.[1]) || 0,
            Number(bounds?.[2]) || 0,
            Number(bounds?.[3]) || 0,
        ].join(',');
    })
        .join('|');
}
/** Position one boundary relative to connected authored content. */
function writeConnectedBoundaryPosition(boundary, kind, nodes, connectedNodes) {
    const position = boundaryPosition(kind, boundary, nodes, connectedNodes);
    return writeBoundaryPosition(boundary, position);
}
/** Calculate one boundary position from authored node extents and graph relations. */
function boundaryPosition(kind, boundary, nodes, connectedNodes) {
    const minX = Math.min(...nodes.map((node) => Number(node.pos[0]) || 0));
    const minY = Math.min(...nodes.map((node) => Number(node.pos[1]) || 0));
    const maxX = Math.max(...nodes.map((node) => (Number(node.pos[0]) || 0) + (Number(node.size[0]) || 0)));
    const maxY = Math.max(...nodes.map((node) => (Number(node.pos[1]) || 0) + (Number(node.size[1]) || 0)));
    const boundaryWidth = Number(boundary.boundingRect?.[2]) ||
        Number(boundary.bounding?.[2]) ||
        Number(boundary.size?.[0]) ||
        DEFAULT_BOUNDARY_WIDTH;
    const boundaryHeight = Number(boundary.boundingRect?.[3]) ||
        Number(boundary.bounding?.[3]) ||
        Number(boundary.size?.[1]) ||
        DEFAULT_BOUNDARY_HEIGHT;
    const alignmentNodes = connectedNodes.length ? connectedNodes : nodes;
    const connectedCenterY = alignmentNodes.reduce((total, node) => total + (Number(node.pos[1]) || 0) + (Number(node.size[1]) || 0) / 2, 0) / alignmentNodes.length;
    const maxBoundaryY = Math.max(minY, maxY - boundaryHeight);
    const boundaryY = Math.min(Math.max(connectedCenterY - boundaryHeight / 2, minY), maxBoundaryY);
    return kind === 'input'
        ? [minX - boundaryWidth - BOUNDARY_GAP, boundaryY]
        : [maxX + BOUNDARY_GAP, boundaryY];
}
/** Write one boundary position without replacing its host-owned vector. */
function writePosition(target, position) {
    if (target[0] === position[0] && target[1] === position[1])
        return false;
    target[0] = position[0];
    target[1] = position[1];
    return true;
}
/** Read a trimmed string array from serialized graph metadata. */
function readStringArray(value) {
    if (!Array.isArray(value))
        return [];
    return value
        .filter((entry) => typeof entry === 'string')
        .map((entry) => entry.trim())
        .filter(Boolean);
}
