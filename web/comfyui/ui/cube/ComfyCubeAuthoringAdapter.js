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
/** Convert a native Comfy selection into a real Cube subgraph node. */
import { buildLinkIndex } from '../graph/GraphQuery.js';
import { isRecord } from '../types/common.js';
const DEFAULT_CUBE_SURFACE_SIZE = [720, 480];
/** Reuse native conversion and retain its generated surface node. */
export class ComfyCubeAuthoringAdapter {
    #graph;
    #subgraphs;
    #convertToSubgraph;
    #canvas;
    #nodeFactory;
    #catalog;
    /** Bind native selection conversion and Cube-node presentation metadata. */
    constructor(options) {
        this.#graph = options.graph;
        this.#subgraphs = options.subgraphs;
        this.#convertToSubgraph = options.convertToSubgraph;
        this.#canvas = options.canvas;
        this.#nodeFactory = options.nodeFactory;
        this.#catalog = options.catalog;
    }
    /** Return the current native selection count. */
    selectedCount() {
        return this.#canvas.selectedItems.size;
    }
    /** Require native incoming and outgoing boundaries before graph mutation. */
    validateSelection() {
        if (this.#canvas.selectedItems.size === 0) {
            throw new Error('Select at least one node to create a SugarCube.');
        }
        const selectedIds = new Set();
        for (const item of this.#canvas.selectedItems) {
            if (!isRecord(item) || item.id == null) {
                throw new Error('Select graph nodes only to create a SugarCube.');
            }
            selectedIds.add(String(item.id));
        }
        let hasInput = false;
        let hasOutput = false;
        const graphLinks = buildLinkIndex(this.#graph).links;
        for (const link of graphLinks) {
            const originId = link.origin_id ?? link.origin;
            const targetId = link.target_id ?? link.target;
            if (originId == null || targetId == null)
                continue;
            const originSelected = selectedIds.has(String(originId));
            const targetSelected = selectedIds.has(String(targetId));
            hasInput ||= !originSelected && targetSelected;
            hasOutput ||= originSelected && !targetSelected;
        }
        if (!hasInput || !hasOutput) {
            const missing = [...(!hasInput ? ['input'] : []), ...(!hasOutput ? ['output'] : [])].join(' and ');
            throw new Error('A SugarCube requires at least one graph input and one graph output. ' +
                `Missing ${missing} boundary after examining ${String(graphLinks.length)} graph links.`);
        }
    }
    /** Convert through Comfy while retaining its native node and boundary links. */
    createFromSelection(identity) {
        this.validateSelection();
        const converted = readConversionResult(this.#convertToSubgraph(new Set(this.#canvas.selectedItems)));
        const metadata = buildCubeMetadata(identity);
        converted.subgraph.name = `Cube: ${identity.defaultAlias}`;
        converted.subgraph.extra = {
            ...(isRecord(converted.subgraph.extra) ? converted.subgraph.extra : {}),
            sugarcubes_kind: 'cube',
            sugarcubes_cube: cloneRecord(metadata),
        };
        const node = this.#nodeFactory.adopt(converted.node, {
            instanceId: identity.instanceId,
            subgraph: converted.subgraph,
            title: identity.defaultAlias,
            position: readPair(converted.node.pos, [0, 0]),
            size: initialCubeSurfaceSize(converted.node.size),
            identity: metadata,
            surface: {},
        });
        this.#subgraphs.set(converted.subgraph.id, converted.subgraph);
        this.#catalog.add(node);
        this.#canvas.selectedItems.clear();
        this.#canvas.updateSelectedItems?.();
        return { node, subgraph: converted.subgraph, identity };
    }
}
/** Validate Comfy's native selection-to-subgraph result. */
function readConversionResult(value) {
    if (!isRecord(value) || !isNativeSubgraph(value.subgraph) || !isConversionNode(value.node)) {
        throw new TypeError('Comfy did not return a valid native subgraph conversion.');
    }
    if (value.node.subgraph !== value.subgraph) {
        throw new TypeError('Comfy returned a wrapper for a different subgraph definition.');
    }
    return { subgraph: value.subgraph, node: value.node };
}
/** Validate the native subgraph members used after conversion. */
function isNativeSubgraph(value) {
    return (isRecord(value) &&
        typeof value.id === 'string' &&
        typeof value.name === 'string' &&
        Array.isArray(value._nodes) &&
        isRecord(value.inputNode) &&
        isRecord(value.outputNode));
}
/** Validate only the disposable native wrapper data used during conversion. */
function isConversionNode(value) {
    return (isRecord(value) &&
        value.id != null &&
        isRecord(value.subgraph) &&
        isNumericVector(value.pos) &&
        isNumericVector(value.size));
}
/** Choose a useful first parent size independently of the native wrapper. */
function initialCubeSurfaceSize(value) {
    const [width, height] = readPair(value, DEFAULT_CUBE_SURFACE_SIZE);
    return [
        Math.max(DEFAULT_CUBE_SURFACE_SIZE[0], width),
        Math.max(DEFAULT_CUBE_SURFACE_SIZE[1], height),
    ];
}
/** Read one finite pair from a host-owned vector. */
function readPair(value, fallback) {
    const x = Number(value[0]);
    const y = Number(value[1]);
    return [Number.isFinite(x) ? x : fallback[0], Number.isFinite(y) ? y : fallback[1]];
}
/** Narrow an unknown host value to a numeric two-dimensional vector. */
function isNumericVector(value) {
    return ((Array.isArray(value) || value instanceof Float32Array || value instanceof Float64Array) &&
        value.length >= 2);
}
/** Build authoritative persisted Cube identity. */
function buildCubeMetadata(identity) {
    return {
        schema: 1,
        kind: 'cube',
        cube_id: identity.cubeId,
        cube_version: '',
        instance_id: identity.instanceId,
        default_alias: identity.defaultAlias,
        instance_alias: identity.defaultAlias,
        description: identity.description,
        ...(identity.targetModel ? { target_model: identity.targetModel } : {}),
        ...(identity.supportedModels.length ? { supported_models: [...identity.supportedModels] } : {}),
    };
}
/** Clone JSON-safe metadata before assigning domain ownership. */
function cloneRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    return isRecord(parsed) ? parsed : {};
}
