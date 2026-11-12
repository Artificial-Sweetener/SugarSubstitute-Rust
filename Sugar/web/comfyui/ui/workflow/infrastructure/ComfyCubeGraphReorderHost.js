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
/** Commit canonical Cube placements through Comfy's native history without replacing live nodes. */
import { readNativeCubePosition, writeNativeCubePosition, restoreNativeCubePosition, } from '../../cube/graph/NativeCubePositions.js';
import { enrichWorkflowPayload } from '../../graph/WorkflowPayloadBuilder.js';
import { isRecord } from '../../types/common.js';
/** Keep serialization, native coordinates and undo transactions at one host boundary. */
export class ComfyCubeGraphReorderHost {
    #options;
    #generation = 0;
    #serializedWorkflow = '';
    #disposed = false;
    #scope = 0;
    #authority = new WeakMap();
    /** Bind the same root graph and history owner used by other Cube mutations. */
    constructor(options) {
        this.#options = options;
    }
    /** Restrict Cube mutations to the root workflow through its established placement policy. */
    get available() {
        return !this.#disposed && this.#options.isCurrentRoot();
    }
    /** Read cached identity without serializing the workflow for every mounted arrow or stack row. */
    get revision() {
        return `${this.available ? 'root' : 'nested'}:${this.#generation}:${this.#serializedWorkflow}`;
    }
    /** Retire cached permissions only at identified native graph and workflow lifecycles. */
    invalidateSnapshot() {
        this.#generation += 1;
    }
    /** Retire gesture ownership on navigation, including leaving and returning to the same root. */
    invalidateScope() {
        this.#scope += 1;
        this.invalidateSnapshot();
    }
    /** Capture complete live workflow data so intervening value or geometry edits make results stale. */
    snapshot() {
        if (this.#disposed)
            throw new Error('Comfy graph snapshot owner has been disposed.');
        const graph = this.#options.graph;
        if (typeof graph.serialize !== 'function')
            throw new TypeError('Comfy workflow serialization is unavailable for Cube reorder.');
        const workflow = graph.serialize();
        if (!isRecord(workflow) || !Array.isArray(workflow.nodes))
            throw new TypeError('Comfy workflow serialization did not contain native nodes.');
        const serializedWorkflow = JSON.stringify(enrichWorkflowPayload(workflow, graph));
        this.#serializedWorkflow = serializedWorkflow;
        const snapshot = {
            serializedWorkflow,
            revision: this.revision,
        };
        this.#authority.set(snapshot, {
            scope: this.#scope,
            nodes: [...this.#nodes()],
            geometry: this.#geometry(true),
            order: this.#geometry(false),
        });
        return snapshot;
    }
    /** Recognize the same native workflow independently of widget edits or cache refreshes. */
    isCurrent(snapshot) {
        const authority = this.#authority.get(snapshot);
        const nodes = this.#nodes();
        return (this.available &&
            authority !== undefined &&
            authority.scope === this.#scope &&
            authority.nodes.length === nodes.length &&
            authority.nodes.every((node) => nodes.includes(node)));
    }
    /** Keep navigation and drag eligibility current independently of measured Cube dimensions. */
    isOrderCurrent(snapshot) {
        return (this.isCurrent(snapshot) && this.#authority.get(snapshot)?.order === this.#geometry(false));
    }
    /** Revalidate all placement inputs before analyzing or committing destinations. */
    isPlacementCurrent(snapshot) {
        return (this.isCurrent(snapshot) && this.#authority.get(snapshot)?.geometry === this.#geometry(true));
    }
    /** Retire native snapshot authority before the runtime's graph objects are released. */
    dispose() {
        this.#disposed = true;
        this.invalidateSnapshot();
    }
    /** Commit geometry atomically while preserving concurrent widget and presentation updates. */
    commit(snapshot, placements) {
        if (!this.isPlacementCurrent(snapshot))
            return false;
        const plan = this.#plan(placements).filter(({ original, destination }) => original[0] !== destination[0] || original[1] !== destination[1]);
        if (plan.length === 0)
            return true;
        const history = this.#options.history;
        history.beforeChange();
        try {
            for (const placement of plan)
                writeNativeCubePosition(placement.node, placement.destination);
            this.#options.reconcileOrder();
            this.#options.graph.afterChange?.();
            this.#options.graph.setDirtyCanvas?.(true, true);
        }
        catch (error) {
            // A host setter may throw after writing. Restore raw coordinates for the entire plan.
            for (const placement of plan)
                restoreNativeCubePosition(placement.node, placement.original);
            history.setDirtyCanvas(true, true);
            throw error;
        }
        finally {
            history.afterChange();
        }
        history.setDirtyCanvas(true, true);
        this.#options.onCommitted(plan.map(({ node }) => node));
        return true;
    }
    /** Read root identities without serializing unrelated workflow state. */
    #nodes() {
        return this.#options.graph._nodes ?? this.#options.graph.nodes ?? [];
    }
    /** Read native Cube order facts and include measured dimensions only for placement validation. */
    #geometry(includeSize) {
        return JSON.stringify(this.#nodes()
            .filter((node) => isRecord(node.properties?.sugarcubes_cube))
            .map((node) => ({
            id: String(node.id),
            instance: node.properties?.sugarcubes_cube,
            title: node.title,
            mode: node.mode,
            position: node.pos ? readNativeCubePosition(node.pos) : null,
            ...(includeSize ? { size: node.size ? readNativeCubePosition(node.size) : null } : {}),
        })));
    }
    /** Resolve all live targets before history begins, rejecting ambiguous or foreign results. */
    #plan(placements) {
        const nodes = this.#options.graph._nodes ?? this.#options.graph.nodes ?? [];
        const byId = new Map(nodes.map((node) => [String(node.id), node]));
        const instanceIds = new Set();
        const nodeIds = new Set();
        return placements.map((placement) => {
            if (instanceIds.has(placement.instanceId) || nodeIds.has(placement.nodeId))
                throw new TypeError('Cube reorder contains duplicate native targets.');
            instanceIds.add(placement.instanceId);
            nodeIds.add(placement.nodeId);
            const node = byId.get(placement.nodeId);
            const metadata = node?.properties?.sugarcubes_cube;
            if (!node || !isRecord(metadata) || metadata.instance_id !== placement.instanceId)
                throw new TypeError('Cube reorder target no longer matches its native identity.');
            return {
                node,
                original: readNativeCubePosition(node.pos),
                destination: readNativeCubePosition(placement.position),
            };
        });
    }
}
