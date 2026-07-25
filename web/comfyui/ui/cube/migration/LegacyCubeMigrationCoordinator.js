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
/** Coordinate two-phase restoration of persisted group-era Cube instances. */
/** Own the lifecycle bridge between pre-configure extraction and post-configure restore. */
export class LegacyCubeMigrationCoordinator {
    #graphBuilder;
    #placement;
    #graph;
    #logger;
    /** Bind focused graph construction, placement, and root-link owners. */
    constructor(options) {
        this.#graphBuilder = options.graphBuilder;
        this.#placement = options.placement;
        this.#graph = options.graph;
        this.#logger = options.logger;
    }
    /** Build semantic native Cubes after Comfy has configured all unrelated root nodes. */
    restore(batch) {
        if (!batch)
            return { migrated: 0, connected: 0, warnings: [] };
        const warnings = [...batch.warnings];
        const placedByKey = new Map();
        for (const plan of batch.plans) {
            try {
                const built = this.#graphBuilder.build(plan);
                const placed = this.#placement.placeBuilt({
                    built,
                    title: plan.title,
                    identity: {
                        cubeId: plan.cubeId,
                        cubeVersion: plan.cubeVersion,
                        instanceId: plan.key,
                        defaultAlias: plan.title,
                        instanceAlias: plan.title,
                        metadata: {},
                    },
                    geometry: { position: plan.position, size: plan.size },
                    recordHistory: false,
                });
                warnings.push(...placed.warnings);
                placedByKey.set(plan.key, placed);
            }
            catch (error) {
                const message = `Failed to migrate legacy Cube '${plan.title}': ${readErrorMessage(error)}`;
                warnings.push(message);
                this.#logger.error(message, error);
            }
        }
        let connected = 0;
        for (const connection of batch.connections) {
            if (this.#restoreConnection(connection, placedByKey, warnings))
                connected += 1;
        }
        for (const warning of warnings)
            this.#logger.warn(`SugarCubes migration: ${warning}`);
        if (placedByKey.size > 0)
            this.#graph.setDirtyCanvas?.(true, true);
        return { migrated: placedByKey.size, connected, warnings };
    }
    /** Resolve and reconnect one former root/marker link through Cube parent slots. */
    #restoreConnection(connection, placedByKey, warnings) {
        const origin = this.#resolveOrigin(connection.origin, placedByKey);
        const target = this.#resolveTarget(connection.target, placedByKey);
        if (!origin || !target) {
            warnings.push(`Could not restore legacy Cube connection of type '${connection.type}'.`);
            return false;
        }
        try {
            origin.node.connect(origin.slot, target.node, target.slot);
            return true;
        }
        catch (error) {
            const message = `Failed to restore migrated Cube connection of type '${connection.type}': ` +
                readErrorMessage(error);
            warnings.push(message);
            this.#logger.error(message, error);
            return false;
        }
    }
    /** Resolve a root node or semantic Cube output to one native origin slot. */
    #resolveOrigin(endpoint, placedByKey) {
        if (endpoint.kind === 'root') {
            const node = this.#graph.getNodeById(endpoint.nodeId);
            return node?.outputs[endpoint.slot] ? { node, slot: endpoint.slot } : null;
        }
        const placed = placedByKey.get(endpoint.cubeKey);
        const slot = placed?.subgraph.outputs.findIndex((output) => output.name === endpoint.name) ?? -1;
        return placed && slot >= 0 ? { node: placed.node, slot } : null;
    }
    /** Resolve a root node or semantic Cube input to one native target slot. */
    #resolveTarget(endpoint, placedByKey) {
        if (endpoint.kind === 'root') {
            const node = this.#graph.getNodeById(endpoint.nodeId);
            return node?.inputs[endpoint.slot] ? { node, slot: endpoint.slot } : null;
        }
        const placed = placedByKey.get(endpoint.cubeKey);
        const slot = placed?.subgraph.inputs.findIndex((input) => input.name === endpoint.name) ?? -1;
        return placed && slot >= 0 ? { node: placed.node, slot } : null;
    }
}
/** Preserve useful error context at the dynamic host boundary. */
function readErrorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}
