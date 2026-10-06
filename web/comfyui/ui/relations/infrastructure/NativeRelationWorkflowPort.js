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
/** Adapt native workflow transactions and metadata to typed relation use cases. */
import { isRecord } from '../../types/common.js';
import { refreshNativeNodeCards } from '../../surface/NativeNodeCardRefresh.js';
import { NativeRelationInventory } from './NativeRelationInventory.js';
import { parseRelationState, RELATION_METADATA_KEY } from './RelationWorkflowMetadata.js';
/** Keep native history and durable workflow state under one integration owner. */
export class NativeRelationWorkflowPort {
    getGraph;
    document;
    getConnections;
    inventorySource;
    #transactionDepth = 0;
    /** Resolve the root workflow lazily so navigation and loading do not change relation scope. */
    constructor(getGraph, document, logger, getCardState, getConnections = () => []) {
        this.getGraph = getGraph;
        this.document = document;
        this.getConnections = getConnections;
        this.inventorySource = new NativeRelationInventory(logger, getCardState);
    }
    /** Read live exposed field capabilities through the shared inventory adapter. */
    inventory() {
        const graph = this.getGraph();
        return this.inventorySource.read(graph, graph ? this.getConnections(graph) : []);
    }
    /** Identify value transactions whose effects have already been reconciled by the application owner. */
    get editing() {
        return this.#transactionDepth > 0;
    }
    /** Parse fresh metadata on each reconciliation so native undo and workflow switching remain authoritative. */
    load() {
        const graph = this.getGraph();
        return parseRelationState(isRecord(graph?.extra) ? graph.extra[RELATION_METADATA_KEY] : null);
    }
    /** Save a detached validated snapshot without modifying Cube definitions or flavors. */
    save(state) {
        const graph = this.getGraph();
        if (!graph)
            return;
        const extra = isRecord(graph.extra) ? graph.extra : {};
        if (JSON.stringify(extra[RELATION_METADATA_KEY]) === JSON.stringify(state))
            return;
        extra[RELATION_METADATA_KEY] = parseRelationState(state);
        graph.extra = extra;
    }
    /** Bracket one user intent and its dependent writes as a single native history transaction. */
    transaction(operation) {
        const graph = this.getGraph(), outer = this.#transactionDepth === 0;
        if (outer)
            graph?.beforeChange?.();
        this.#transactionDepth++;
        try {
            operation();
        }
        finally {
            try {
                if (outer)
                    graph?.afterChange?.();
            }
            finally {
                this.#transactionDepth--;
            }
        }
    }
    /** Invalidate projected native widgets after effective values change. */
    refresh(nodeIds) {
        if (!nodeIds.length)
            return;
        this.getGraph()?.setDirtyCanvas?.(true, true);
        const affected = new Set(nodeIds);
        const nodes = [
            ...new Set([...this.inventorySource.owners.values()].flatMap((owner) => affected.has(owner.nodeId) ? [owner.face, owner.node] : [])),
        ];
        refreshNativeNodeCards(this.document, { kind: 'nodes', nodes });
    }
}
