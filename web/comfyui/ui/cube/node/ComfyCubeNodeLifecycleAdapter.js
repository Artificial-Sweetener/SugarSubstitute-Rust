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
/** Keep root Cube identity and presentation aligned with Comfy's workflow graph. */
import { isRecord } from '../../types/common.js';
import { labelEmptyCubeBoundaryAffordances } from '../geometry/NativeCubeBoundaryLayout.js';
import { isCubeNode, isDraftCubeNode, requireCubeIdentity, } from './ComfyCubeNodeFactory.js';
import { readInstanceId } from './CubeNodeCatalog.js';
/** Own host lifecycle hooks, pasted identity, and the non-authoritative Cube index. */
export class ComfyCubeNodeLifecycleAdapter {
    #graph;
    #catalog;
    #inventory;
    #events;
    #createInstanceId;
    #logger;
    #nodePackMetadata;
    #unsubscribeNodePackMetadata;
    #previousConfigure;
    #previousNodeAdded;
    #previousNodeRemoved;
    #configureHook;
    #nodeAddedHook;
    #nodeRemovedHook;
    /** Install graph and completed-canvas-change synchronization around prior callbacks. */
    constructor(options) {
        this.#graph = options.graph;
        this.#catalog = options.catalog;
        this.#inventory = options.inventory;
        this.#events = options.events;
        this.#createInstanceId = options.createInstanceId;
        this.#logger = options.logger;
        this.#nodePackMetadata = options.nodePackMetadata;
        this.#previousConfigure = options.graph.onConfigure ?? null;
        this.#previousNodeAdded = options.graph.onNodeAdded ?? null;
        this.#previousNodeRemoved = options.graph.onNodeRemoved ?? null;
        this.#configureHook = (data) => {
            this.#previousConfigure?.call(this.#graph, data);
            this.#reconcile();
        };
        this.#nodeAddedHook = (node) => {
            this.#previousNodeAdded?.call(this.#graph, node);
            if (isCubeNode(node))
                this.#reconcile();
        };
        this.#nodeRemovedHook = (node) => {
            this.#previousNodeRemoved?.call(this.#graph, node);
            if (isCubeNode(node) && this.#catalog.get(readInstanceId(node)) === node) {
                this.#catalog.remove(readInstanceId(node));
            }
        };
        options.graph.onConfigure = this.#configureHook;
        options.graph.onNodeAdded = this.#nodeAddedHook;
        options.graph.onNodeRemoved = this.#nodeRemovedHook;
        options.events.addEventListener('litegraph:canvas', this.#handleCanvasChange);
        this.#unsubscribeNodePackMetadata = options.nodePackMetadata.subscribe(() => this.#reconcile());
        this.#reconcile();
    }
    /** Restore callbacks when the graph-bound runtime is replaced. */
    dispose() {
        if (this.#graph.onConfigure === this.#configureHook) {
            this.#graph.onConfigure = this.#previousConfigure;
        }
        if (this.#graph.onNodeAdded === this.#nodeAddedHook) {
            this.#graph.onNodeAdded = this.#previousNodeAdded;
        }
        if (this.#graph.onNodeRemoved === this.#nodeRemovedHook) {
            this.#graph.onNodeRemoved = this.#previousNodeRemoved;
        }
        this.#events.removeEventListener('litegraph:canvas', this.#handleCanvasChange);
        this.#unsubscribeNodePackMetadata();
    }
    /** Reconcile only after Comfy has configured all nodes created by one canvas operation. */
    #handleCanvasChange = (event) => {
        if (!isRecord(event) || !isRecord(event.detail) || event.detail.subType !== 'after-change') {
            return;
        }
        try {
            this.#reconcile();
        }
        catch (error) {
            this.#logger.error('SugarCubes could not reconcile Cube nodes after a graph change.', {
                error,
            });
            throw error;
        }
    };
    /** Assign fresh per-instance identities to copied Cubes before rebuilding the index. */
    #reconcile() {
        const seen = new Map();
        const values = this.#inventory.snapshot().rootCubes;
        for (const value of values) {
            if (!isCubeNode(value))
                continue;
            this.#nodePackMetadata.apply(value);
            if (isDraftCubeNode(value))
                labelEmptyCubeBoundaryAffordances(value.subgraph);
            let instanceId = readInstanceId(value);
            const existing = seen.get(instanceId);
            if (existing && existing !== value) {
                instanceId = this.#assignFreshInstanceId(value, seen);
            }
            seen.set(instanceId, value);
        }
        this.#catalog.replace(values);
    }
    /** Replace one copied identity without changing its Cube definition identity. */
    #assignFreshInstanceId(node, reserved) {
        for (let attempt = 0; attempt < 32; attempt += 1) {
            const candidate = this.#createInstanceId().trim();
            if (!candidate || reserved.has(candidate) || this.#catalog.get(candidate))
                continue;
            const prior = readInstanceId(node);
            requireCubeIdentity(node).instance_id = candidate;
            this.#logger.debug('SugarCubes assigned a fresh identity to a copied Cube node.', {
                node_id: String(node.id),
                prior_instance_id: prior,
                instance_id: candidate,
            });
            return candidate;
        }
        throw new Error(`Could not allocate a unique identity for copied Cube '${String(node.id)}'.`);
    }
}
