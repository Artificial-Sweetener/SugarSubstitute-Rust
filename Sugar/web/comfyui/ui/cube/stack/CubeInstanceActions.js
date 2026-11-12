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
/** Own graph-local Cube instance changes through the native history and catalog boundaries. */
import { isCubeNode, requireCubeIdentity } from '../node/ComfyCubeNodeFactory.js';
import { captureNativeCubeConnections, restoreNativeCubeConnections, } from '../graph/NativeCubeConnections.js';
/** Keep definition identity intact while applying undoable changes to one live workflow instance. */
export class CubeInstanceActions {
    #options;
    /** Bind one graph lifetime and its authoritative mutation collaborators. */
    constructor(options) {
        this.#options = options;
    }
    /** Reject stale menu targets before accepting synchronous or delayed user intent. */
    require(instanceId) {
        this.#options.scope.assertCurrentRoot('edited');
        if (this.#options.isReordering())
            throw new Error('Wait for Cube reordering to finish.');
        const node = this.#options.nodes.get(instanceId);
        if (!node || this.#options.graph.getNodeById(node.id) !== node)
            throw new Error('The Cube is no longer in the current workflow.');
        return node;
    }
    /** Change the instance label without renaming the shared definition or saved Cube. */
    rename(instanceId, requested) {
        const node = this.require(instanceId);
        const title = requested.trim();
        if (!title)
            throw new Error('Cube name is required.');
        if (node.title === title)
            return;
        const previous = node.title;
        const identity = requireCubeIdentity(node);
        const alias = identity.instance_alias;
        this.#change(node, () => {
            node.title = title;
            identity.instance_alias = title;
        }, () => {
            if (previous === undefined)
                delete node.title;
            else
                node.title = previous;
            identity.instance_alias = alias;
        });
    }
    /** Toggle native bypass while restoring an existing muted mode when bypass is removed. */
    toggleBypass(instanceId) {
        const node = this.require(instanceId);
        const mode = node.mode;
        const prior = node.properties.sugarcubes_before_bypass_mode;
        this.#change(node, () => {
            if (mode === 4) {
                node.mode = prior === 2 ? 2 : 0;
                delete node.properties.sugarcubes_before_bypass_mode;
            }
            else {
                node.properties.sugarcubes_before_bypass_mode = mode ?? 0;
                node.mode = 4;
            }
        }, () => {
            node.mode = mode;
            if (prior === undefined)
                delete node.properties.sugarcubes_before_bypass_mode;
            else
                node.properties.sugarcubes_before_bypass_mode = prior;
        });
    }
    /** Persist the workflow-local output policy consumed by the execution sink owner. */
    toggleOutputPersistence(instanceId) {
        const node = this.require(instanceId);
        const prior = node.properties.sugarcubes_save_outputs;
        this.#change(node, () => {
            node.properties.sugarcubes_save_outputs = prior === false;
        }, () => {
            if (prior === undefined)
                delete node.properties.sugarcubes_save_outputs;
            else
                node.properties.sugarcubes_save_outputs = prior;
        });
    }
    /** Clone native instance values with independent identity and append without copying external links. */
    duplicate(instanceId) {
        const source = this.require(instanceId);
        if (typeof source.clone !== 'function')
            throw new Error('Native Cube cloning is unavailable.');
        const clone = source.clone();
        if (!isCubeNode(clone) ||
            clone === source ||
            clone.properties === source.properties ||
            requireCubeIdentity(clone) === requireCubeIdentity(source))
            throw new Error('Comfy returned an invalid Cube clone.');
        const id = this.#options.createId().trim();
        if (!id || this.#options.nodes.get(id) || this.#options.graph.getNodeById(id))
            throw new Error('Could not allocate a fresh Cube instance identity.');
        clone.id = id;
        requireCubeIdentity(clone).instance_id = id;
        const title = this.#duplicateTitle(source.title ?? source.subgraph.name);
        clone.title = title;
        requireCubeIdentity(clone).instance_alias = title;
        const x = Math.max(...this.#options.nodes.list().map((node) => Number(node.pos[0]) + Number(node.size[0])));
        if (!Number.isFinite(x))
            throw new Error('Cube stack contains invalid native geometry.');
        clone.pos[0] = x + 24;
        clone.pos[1] = source.pos[1] ?? 0;
        this.#change(clone, () => {
            this.#options.graph.add(clone);
            this.#options.nodes.add(clone);
        }, () => {
            if (this.#options.graph.getNodeById(clone.id) === clone)
                this.#options.graph.remove(clone);
            this.#options.nodes.remove(id);
        });
        return clone;
    }
    /** Remove this instance directly and compensate native connections if the host rejects removal. */
    remove(instanceId) {
        const node = this.require(instanceId);
        const connections = captureNativeCubeConnections(this.#options.graph, [node]);
        this.#change(node, () => {
            this.#options.graph.remove(node);
            this.#options.nodes.remove(instanceId);
        }, () => {
            if (this.#options.graph.getNodeById(node.id) !== node)
                this.#options.graph.add(node);
            this.#options.nodes.add(node);
            restoreNativeCubeConnections(this.#options.graph, connections);
        });
    }
    /** Allocate a readable copy label from the current native instance titles. */
    #duplicateTitle(title) {
        const taken = new Set(this.#options.nodes.list().map((node) => node.title?.trim().toLowerCase()));
        for (let index = 2; index < 10000; index++) {
            const candidate = `${title} ${index}`;
            if (!taken.has(candidate.toLowerCase()))
                return candidate;
        }
        throw new Error('Could not allocate a Cube copy name.');
    }
    /** Publish one coherent native transaction, including compensation before history completion. */
    #change(node, work, restore) {
        const { nodes, history } = this.#options;
        history.beforeChange();
        try {
            nodes.batch(() => {
                try {
                    work();
                }
                catch (error) {
                    try {
                        restore();
                    }
                    catch (failure) {
                        throw new AggregateError([error, failure], 'Cube instance change and compensation failed.');
                    }
                    throw error;
                }
                nodes.changed(node);
            });
        }
        finally {
            history.setDirtyCanvas(true, true);
            history.afterChange();
        }
    }
}
