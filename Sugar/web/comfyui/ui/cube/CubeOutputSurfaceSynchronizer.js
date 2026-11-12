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
/** Keep Cube-owned native output labels concise as their interface changes. */
import { deriveCubeOutputSurfaceNames } from './CubeOutputSurfaceNames.js';
/** Synchronize Cube output labels after host graph changes and node registration. */
export class CubeOutputSurfaceSynchronizer {
    #nodes;
    #events;
    #unsubscribeNodes;
    #synchronizing = false;
    /** Bind the Cube catalog and host graph-change event boundary. */
    constructor(nodes, events) {
        this.#nodes = nodes;
        this.#events = events;
        this.#unsubscribeNodes = nodes.subscribe(() => this.#synchronize());
        this.#events.addEventListener('litegraph:canvas', this.#handleGraphChange);
        this.#synchronize();
    }
    /** Release subscriptions when the graph-bound runtime is replaced. */
    dispose() {
        this.#unsubscribeNodes();
        this.#events.removeEventListener('litegraph:canvas', this.#handleGraphChange);
    }
    /** Respond only after Comfy completes a graph mutation. */
    #handleGraphChange = (event) => {
        if (!(event instanceof CustomEvent) || event.detail?.subType !== 'after-change')
            return;
        this.#synchronize();
    };
    /** Normalize every registered Cube without recursively notifying the catalog. */
    #synchronize() {
        if (this.#synchronizing)
            return;
        this.#synchronizing = true;
        try {
            for (const node of this.#nodes.list()) {
                if (synchronizeCubeOutputSurfaceNames(node))
                    this.#nodes.changed(node);
            }
        }
        finally {
            this.#synchronizing = false;
        }
    }
}
/** Apply the authoritative surface names to both native sides of one Cube boundary. */
export function synchronizeCubeOutputSurfaceNames(node) {
    const count = Math.max(node.subgraph.outputs.length, node.outputs.length);
    if (count === 0)
        return false;
    const descriptors = Array.from({ length: count }, (_, index) => ({
        name: node.subgraph.outputs[index]?.name ?? node.outputs[index]?.name,
        type: node.subgraph.outputs[index]?.type ?? node.outputs[index]?.type,
    }));
    const names = deriveCubeOutputSurfaceNames(descriptors);
    let changed = false;
    for (const [index, name] of names.entries()) {
        const subgraphOutput = node.subgraph.outputs[index];
        const nodeOutput = node.outputs[index];
        if (subgraphOutput && subgraphOutput.name !== name) {
            subgraphOutput.name = name;
            changed = true;
        }
        if (nodeOutput && nodeOutput.name !== name) {
            nodeOutput.name = name;
            changed = true;
        }
    }
    return changed;
}
