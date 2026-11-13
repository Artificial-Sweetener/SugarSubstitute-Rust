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
/** Place one picker Cube after a live Cube without creating persisted links. */
import { CubeNodeInsertAfterCoordinator } from '../cube/node/CubeNodeInsertAfterCoordinator.js';
/** Own picker preparation, atomic insertion, downstream displacement, and rollback. */
export class CubePickerInsertAfterService {
    #placement;
    #getRuntime;
    /** Bind picker preparation to the current root-graph runtime. */
    constructor(options) {
        this.#placement = options.placement;
        this.#getRuntime = options.getRuntime;
    }
    /** Insert one Cube after the source instance and return its graph-owned node. */
    insert(sourceInstanceId, type) {
        const runtime = this.#getRuntime();
        const source = runtime.nodes.get(sourceInstanceId);
        if (!source)
            throw new Error(`Cube instance '${sourceInstanceId}' is no longer available.`);
        const positions = new CubeNodeInsertAfterCoordinator(runtime.nodes);
        const prepared = this.#placement.prepare(type, positions.insertionOrigin(source));
        const registration = runtime.registerSubgraphs(prepared.payload);
        try {
            const placed = runtime.placement.placeBatch([{ payload: prepared.payload, options: prepared.options }], ([inserted]) => {
                if (!inserted)
                    throw new Error('SugarCube insertion did not produce a node.');
                positions.insertAfter(source, inserted.node);
            })[0];
            if (!placed)
                throw new Error('SugarCube insertion did not produce a node.');
            this.#placement.reportWarnings(prepared.descriptor.cubeId, registration, placed);
            return placed.node;
        }
        catch (error) {
            runtime.discardSubgraphs(registration.createdIds);
            throw error;
        }
    }
}
