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
/** Validate native graph construction capabilities before they enter Cube orchestration. */
import { isRecord } from '../types/common.js';
/** Validate the node surface required by native Cube graph construction. */
export function isNativeGraphNode(value) {
    if (!isRecord(value))
        return false;
    value.properties = isRecord(value.properties) ? value.properties : {};
    return (isNumericVector(value.pos) &&
        isNumericVector(value.size) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        typeof value.connect === 'function');
}
/** Validate the Comfy subgraph API used by Cube assembly and serialization. */
export function isNativeCubeSubgraph(value) {
    return (isRecord(value) &&
        typeof value.id === 'string' &&
        typeof value.name === 'string' &&
        Array.isArray(value._nodes) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        isRecord(value.inputNode) &&
        isRecord(value.outputNode) &&
        typeof value.add === 'function' &&
        typeof value.remove === 'function' &&
        typeof value.addInput === 'function' &&
        typeof value.addOutput === 'function' &&
        typeof value.configure === 'function');
}
/** Require Comfy's native definition map and lifecycle hook surface. */
export function requireRuntimeGraph(graph) {
    if (!(graph.subgraphs instanceof Map)) {
        throw new TypeError('LGraph.subgraphs is unavailable.');
    }
    return graph;
}
/** Accept LiteGraph's array and typed-array geometry vectors. */
export function isNumericVector(value) {
    return Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array;
}
/** Require the exact host capabilities used by the Nodes 1.0 Cube node face. */
export function requireLiteGraphCubeNodeCanvas(canvas) {
    if (!(canvas.canvas instanceof HTMLCanvasElement) ||
        typeof canvas.drawNode !== 'function' ||
        typeof canvas.processWidgetClick !== 'function' ||
        !isNumericVector(canvas.graph_mouse)) {
        throw new TypeError('Comfy Nodes 1.0 Cube-node integration is unavailable.');
    }
    return canvas;
}
/** Validate a native node capable of owning one restored graph connection. */
export function isConnectableNode(value) {
    return (isRecord(value) &&
        Array.isArray(value.inputs) &&
        Array.isArray(value.outputs) &&
        typeof value.connect === 'function');
}
