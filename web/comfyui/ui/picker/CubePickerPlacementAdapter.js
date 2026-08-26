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
/** Adapt cached picker payloads to detached Cube construction. */
import { planPlacementGeometry } from '../geometry/PlacementGeometryPlanner.js';
import { resolveRendererGeometryPolicy, } from '../geometry/RendererGeometryPolicy.js';
import { prepareGraphInsertionPayload } from '../import/PlacementPayload.js';
/** Own synchronous prepared-cache resolution and detached Cube construction. */
export class CubePickerPlacementAdapter {
    #registry;
    #getRuntime;
    #getLiteGraph;
    #getNodeRenderer;
    #logger;
    /** Bind host-neutral catalog state to the graph-bound construction use case. */
    constructor(options) {
        this.#registry = options.registry;
        this.#getRuntime = options.getRuntime;
        this.#getLiteGraph = options.getLiteGraph;
        this.#getNodeRenderer = options.getNodeRenderer;
        this.#logger = options.logger;
    }
    /** Construct one final native Cube node without adding it to a graph. */
    create(type, position = null) {
        const descriptor = this.#registry.descriptor(type);
        const cached = this.#registry.preparedPayload(type);
        if (!descriptor || !cached) {
            throw new Error(`SugarCube picker type '${type}' is no longer available.`);
        }
        const runtime = this.#getRuntime();
        runtime.graphScope.assertCurrentRoot('placed');
        const liteGraph = this.#getLiteGraph();
        if (!liteGraph || typeof liteGraph.createNode !== 'function') {
            throw new Error('LiteGraph is unavailable for SugarCube placement.');
        }
        const insertionPayload = prepareGraphInsertionPayload(cached, {
            ...(position ? { targetOrigin: position } : {}),
            remapInstanceIds: true,
        });
        if (!insertionPayload?.cube) {
            throw new Error(`SugarCube '${descriptor.cubeId}' has no valid placement identity.`);
        }
        const geometryPolicy = resolveRendererGeometryPolicy(liteGraph, this.#getNodeRenderer());
        const payload = planPlacementGeometry(insertionPayload, geometryPolicy);
        const registration = runtime.registerSubgraphs(payload);
        try {
            for (const warning of registration.warnings) {
                this.#logger.warn('SugarCubes picker registered a nested definition with a warning.', {
                    cubeId: descriptor.cubeId,
                    warning,
                });
            }
            const constructed = runtime.construction.construct(payload, {
                instanceAlias: descriptor.displayName,
                ...(position ? { position } : {}),
            });
            for (const warning of constructed.warnings) {
                this.#logger.warn('SugarCubes picker constructed a Cube with a warning.', {
                    cubeId: descriptor.cubeId,
                    warning,
                });
            }
            return constructed.node;
        }
        catch (error) {
            runtime.discardSubgraphs(registration.createdIds);
            throw error;
        }
    }
}
