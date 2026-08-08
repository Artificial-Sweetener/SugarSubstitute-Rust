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
import { isRecord } from '../types/common.js';
import { isDraftCubeNode, } from './node/ComfyCubeNodeFactory.js';
import { writeCubeDefinitionIdentity } from './node/CubeDefinitionIdentityWriter.js';
import { resolveCubeDefinitionDescription } from './node/CubeDefinitionIdentityWriter.js';
import { ComfyCubeGraphScope } from './placement/ComfyCubeGraphScope.js';
const DEFAULT_CUBE_SURFACE_SIZE = [720, 480];
/** Reuse native conversion and retain its generated surface node. */
export class ComfyCubeAuthoringAdapter {
    #graph;
    #subgraphs;
    #convertToSubgraph;
    #canvas;
    #nodeFactory;
    #catalog;
    #graphScope;
    #createEmptySubgraph;
    #getDraftPosition;
    /** Bind native selection conversion and Cube-node presentation metadata. */
    constructor(options) {
        this.#graph = options.graph;
        this.#subgraphs = options.subgraphs;
        this.#convertToSubgraph = options.convertToSubgraph;
        this.#canvas = options.canvas;
        this.#nodeFactory = options.nodeFactory;
        this.#catalog = options.catalog;
        const fallbackRoot = options.graph ?? {};
        this.#graphScope =
            options.graphScope ??
                new ComfyCubeGraphScope(fallbackRoot, () => options.canvas.graph ?? fallbackRoot);
        this.#createEmptySubgraph =
            options.createEmptySubgraph ??
                (() => {
                    throw new Error('Native empty Cube draft creation is unavailable.');
                });
        this.#getDraftPosition = options.getDraftPosition ?? (() => [0, 0]);
    }
    /** Return the current native selection count. */
    selectedCount() {
        return this.#canvas.selectedItems.size;
    }
    /** Require a non-empty native node selection before graph mutation. */
    validateSelection() {
        this.#graphScope.assertCurrentRoot('created');
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
        if (!selectedIds.size)
            throw new Error('Select graph nodes only to create a SugarCube.');
    }
    /** Create an empty graph-only Cube draft with its real native subgraph boundary. */
    createEmptyDraft(identity) {
        this.#graphScope.assertCurrentRoot('created');
        const metadata = buildDraftMetadata(identity);
        const subgraph = this.#createEmptySubgraph(`Cube: ${identity.defaultAlias}`, resolveCubeDefinitionDescription(metadata));
        markSubgraphAsDraft(subgraph, metadata);
        const node = this.#nodeFactory.create({
            instanceId: identity.instanceId,
            subgraph,
            title: identity.defaultAlias,
            position: this.#getDraftPosition(),
            size: DEFAULT_CUBE_SURFACE_SIZE,
            identity: metadata,
            surface: {},
            kind: 'draft',
        });
        if (typeof this.#graph?.add !== 'function') {
            throw new Error('Native root graph insertion is unavailable.');
        }
        this.#graph.add(node);
        this.#subgraphs.set(subgraph.id, subgraph);
        this.#catalog.add(node);
        return { node, subgraph, identity };
    }
    /** Convert through Comfy while retaining its authoritative native boundary links. */
    createDraftFromSelection(identity) {
        this.validateSelection();
        const converted = readConversionResult(this.#convertToSubgraph(new Set(this.#canvas.selectedItems)));
        return this.#finalizeDraft(converted.node, identity);
    }
    /** Require exactly one unconverted native subgraph. */
    validateSelectedSubgraph() {
        this.#graphScope.assertCurrentRoot('converted');
        readSelectedSubgraphNode(this.#canvas.selectedItems);
    }
    /** Mark one existing native subgraph as a graph-only draft without changing its interface. */
    createDraftFromSelectedSubgraph(identity) {
        this.#graphScope.assertCurrentRoot('converted');
        const node = readSelectedSubgraphNode(this.#canvas.selectedItems);
        return this.#finalizeDraft(node, identity);
    }
    /** Promote exactly one saved draft without rebuilding its internal native subgraph. */
    promoteDraft(instanceId, identity) {
        const node = this.#catalog.get(instanceId);
        if (!node || !isDraftCubeNode(node)) {
            throw new Error('The selected Cube draft is no longer available.');
        }
        const metadata = buildCubeMetadata(identity);
        node.subgraph.name = `Cube: ${identity.defaultAlias}`;
        writeCubeDefinitionIdentity(node.subgraph, 'cube', metadata);
        this.#nodeFactory.adopt(node, {
            instanceId: identity.instanceId,
            subgraph: node.subgraph,
            title: identity.defaultAlias,
            position: readPair(node.pos, [0, 0]),
            size: initialCubeSurfaceSize(node.size),
            identity: metadata,
            surface: {},
        });
        this.#catalog.changed(node);
        return { node, subgraph: node.subgraph, identity };
    }
    /** Restore a failed first-save promotion to its workflow-only draft state. */
    restoreDraft(instanceId) {
        const node = this.#catalog.get(instanceId);
        if (!node)
            return;
        const identity = {
            instanceId,
            defaultAlias: node.title?.trim() || 'Untitled Cube',
        };
        const metadata = buildDraftMetadata(identity);
        markSubgraphAsDraft(node.subgraph, metadata);
        this.#nodeFactory.adopt(node, {
            instanceId,
            subgraph: node.subgraph,
            title: identity.defaultAlias,
            position: readPair(node.pos, [0, 0]),
            size: initialCubeSurfaceSize(node.size),
            identity: metadata,
            surface: {},
            kind: 'draft',
        });
        this.#catalog.changed(node);
    }
    /** Apply Cube identity and presentation metadata to a native subgraph node. */
    #finalizeDraft(nativeNode, identity) {
        const metadata = buildDraftMetadata(identity);
        nativeNode.subgraph.name = `Cube: ${identity.defaultAlias}`;
        markSubgraphAsDraft(nativeNode.subgraph, metadata);
        const node = this.#nodeFactory.adopt(nativeNode, {
            instanceId: identity.instanceId,
            subgraph: nativeNode.subgraph,
            title: identity.defaultAlias,
            position: readPair(nativeNode.pos, [0, 0]),
            size: initialCubeSurfaceSize(nativeNode.size),
            identity: metadata,
            surface: {},
            kind: 'draft',
        });
        this.#subgraphs.set(nativeNode.subgraph.id, nativeNode.subgraph);
        this.#catalog.add(node);
        this.#canvas.selectedItems.clear();
        this.#canvas.updateSelectedItems?.();
        return { node, subgraph: nativeNode.subgraph, identity };
    }
}
/** Persist the draft marker where both native Comfy renderers can read it. */
function markSubgraphAsDraft(subgraph, metadata) {
    writeCubeDefinitionIdentity(subgraph, 'cube_draft', metadata);
}
/** Infer editor-local boundaries when Comfy stores them only on native node slots. */
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
/** Validate the one selected native subgraph node that will be promoted in place. */
function readSelectedSubgraphNode(items) {
    if (items.size !== 1) {
        throw new Error('Select exactly one subgraph node to convert it to a SugarCube.');
    }
    const selected = items.values().next().value;
    if (!isNativeSubgraphNode(selected)) {
        throw new Error('Select a native Comfy subgraph node to convert it to a SugarCube.');
    }
    if (selected.properties.sugarcubes_kind === 'cube' ||
        selected.properties.sugarcubes_kind === 'cube_draft') {
        throw new Error('The selected subgraph is already a SugarCube.');
    }
    if (selected.isSubgraphNode.call(selected) !== true) {
        throw new Error('Select a native Comfy subgraph node to convert it to a SugarCube.');
    }
    if (!Array.isArray(selected.subgraph.inputs) || !Array.isArray(selected.subgraph.outputs)) {
        throw new Error('The selected subgraph does not expose native input and output boundaries.');
    }
    return selected;
}
/** Narrow a selected host value to the native subgraph members used for promotion. */
function isNativeSubgraphNode(value) {
    return (isConversionNode(value) &&
        isRecord(value.properties) &&
        typeof value.isSubgraphNode === 'function');
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
/** Build workflow-only draft metadata with no persistent Cube id. */
function buildDraftMetadata(identity) {
    return {
        schema: 1,
        kind: 'draft',
        instance_id: identity.instanceId,
        default_alias: identity.defaultAlias,
        instance_alias: identity.defaultAlias,
    };
}
