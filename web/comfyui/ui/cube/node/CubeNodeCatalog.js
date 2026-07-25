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
/** Index live native Cube nodes without owning their graph lifecycle. */
import { isCubeNode, requireCubeIdentity } from './ComfyCubeNodeFactory.js';
/** Provide stable instance lookup while the root graph remains authoritative. */
export class CubeNodeCatalog {
    #nodes = new Map();
    #listeners = new Set();
    /** Index one graph-owned Cube node. */
    add(node) {
        const instanceId = readInstanceId(node);
        const existing = this.#nodes.get(instanceId);
        if (existing === node)
            return;
        if (existing) {
            throw new Error(`Cube node '${instanceId}' already exists.`);
        }
        this.#nodes.set(instanceId, node);
        this.#notify();
    }
    /** Rebuild the index from Comfy's authoritative root-node collection. */
    replace(nodes) {
        const replacement = new Map();
        for (const value of nodes) {
            if (!isCubeNode(value))
                continue;
            const instanceId = readInstanceId(value);
            if (replacement.has(instanceId)) {
                throw new Error(`Workflow contains duplicate Cube node '${instanceId}'.`);
            }
            replacement.set(instanceId, value);
        }
        this.#nodes.clear();
        for (const [instanceId, node] of replacement)
            this.#nodes.set(instanceId, node);
        this.#notify();
    }
    /** Stop indexing one instance after Comfy removes its native node. */
    remove(instanceId) {
        const node = this.#nodes.get(instanceId) ?? null;
        if (node) {
            this.#nodes.delete(instanceId);
            this.#notify();
        }
        return node;
    }
    /** Return one graph-owned Cube node by stable instance identity. */
    get(instanceId) {
        return this.#nodes.get(instanceId) ?? null;
    }
    /** Notify presentation after a native node's Cube-owned face state changes. */
    changed(node) {
        if (this.#nodes.get(readInstanceId(node)) === node)
            this.#notify();
    }
    /** Return graph-owned Cube nodes in stable discovery order. */
    list() {
        return [...this.#nodes.values()];
    }
    /** Observe index and face-state changes. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Notify subscribers after one coherent catalog change. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
/** Require the stable extension instance identity stored on one native node. */
export function readInstanceId(node) {
    const value = requireCubeIdentity(node).instance_id;
    const instanceId = typeof value === 'string' ? value.trim() : '';
    if (!instanceId) {
        throw new TypeError(`Cube node '${String(node.id)}' has no stable instance identity.`);
    }
    return instanceId;
}
