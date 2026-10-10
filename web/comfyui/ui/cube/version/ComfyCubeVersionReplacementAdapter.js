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
/** Replace one live Cube node while preserving compatible external graph connections. */
import { readInstanceId } from '../node/CubeNodeCatalog.js';
import { readNativeCubeConnection } from '../graph/NativeCubeConnections.js';
/** Own the single history boundary, rollback, catalog, selection, and reconnection mutation. */
export class ComfyCubeVersionReplacementAdapter {
    #graph;
    #catalog;
    #history;
    #selection;
    #logger;
    /** Bind graph mutation collaborators without taking version or transfer policy. */
    constructor(options) {
        this.#graph = options.graph;
        this.#catalog = options.catalog;
        this.#history = options.history;
        this.#selection = options.selection;
        this.#logger = options.logger;
    }
    /** Atomically replace one Cube after proving all connected boundaries remain available. */
    replace(request) {
        const connections = captureConnections(this.#graph, request);
        let sourceRemoved = false;
        let targetInserted = false;
        this.#history.beforeChange?.();
        try {
            this.#remove(request.source);
            sourceRemoved = true;
            this.#insert(request.target);
            targetInserted = true;
            reconnectTarget(request.target, connections);
            this.#selection.replace(request.source, request.target);
            this.#history.setDirtyCanvas?.(true, true);
        }
        catch (error) {
            if (sourceRemoved) {
                try {
                    if (targetInserted)
                        this.#remove(request.target);
                    this.#insert(request.source);
                    reconnectSource(request.source, connections);
                    this.#selection.replace(request.target, request.source);
                }
                catch (rollbackError) {
                    this.#logger.error('SugarCubes: Cube version rollback failed.', rollbackError);
                }
            }
            throw error;
        }
        finally {
            this.#history.afterChange?.();
        }
    }
    /** Remove one graph node and make catalog cleanup idempotent with host lifecycle hooks. */
    #remove(node) {
        this.#graph.remove(node);
        this.#catalog.remove(readInstanceId(node));
    }
    /** Insert one graph node and make catalog registration idempotent with host lifecycle hooks. */
    #insert(node) {
        this.#graph.add(node);
        this.#catalog.add(node);
    }
}
/** Capture and validate every live connection before graph mutation begins. */
function captureConnections(graph, request) {
    const inputs = [];
    request.source.inputs.forEach((input, sourceSlot) => {
        if (input.link == null)
            return;
        const boundaryId = request.sourceBoundaries.inputIdBySlot.get(sourceSlot);
        const targetSlot = boundaryId
            ? request.targetBoundaries.inputSlotById.get(boundaryId)
            : undefined;
        if (targetSlot == null)
            throw incompatibleBoundary('input', input.name, boundaryId);
        const connection = readNativeCubeConnection(graph, input.link);
        const outside = connection.source;
        inputs.push({
            outside,
            outsideSlot: connection.sourceSlot,
            sourceSlot,
            targetSlot,
        });
    });
    const outputs = [];
    request.source.outputs.forEach((output, sourceSlot) => {
        for (const linkId of output.links ?? []) {
            const boundaryId = request.sourceBoundaries.outputIdBySlot.get(sourceSlot);
            const targetSlot = boundaryId
                ? request.targetBoundaries.outputSlotById.get(boundaryId)
                : undefined;
            if (targetSlot == null)
                throw incompatibleBoundary('output', output.name, boundaryId);
            const connection = readNativeCubeConnection(graph, linkId);
            const outside = connection.target;
            outputs.push({
                outside,
                outsideSlot: connection.targetSlot,
                sourceSlot,
                targetSlot,
            });
        }
    });
    return { inputs, outputs };
}
/** Reconnect compatible boundaries to the replacement node. */
function reconnectTarget(target, connections) {
    for (const connection of connections.inputs) {
        connection.outside.connect(connection.outsideSlot, target, connection.targetSlot);
    }
    for (const connection of connections.outputs) {
        target.connect(connection.targetSlot, connection.outside, connection.outsideSlot);
    }
}
/** Restore original slot indices when a replacement mutation rolls back. */
function reconnectSource(source, connections) {
    for (const connection of connections.inputs) {
        connection.outside.connect(connection.outsideSlot, source, connection.sourceSlot);
    }
    for (const connection of connections.outputs) {
        source.connect(connection.sourceSlot, connection.outside, connection.outsideSlot);
    }
}
/** Explain a fail-closed version switch without dropping a user's connection. */
function incompatibleBoundary(kind, name, id) {
    const label = typeof name === 'string' && name.trim() ? name.trim() : id || 'unknown';
    return new Error(`Cube version cannot preserve connected ${kind} '${label}'.`);
}
