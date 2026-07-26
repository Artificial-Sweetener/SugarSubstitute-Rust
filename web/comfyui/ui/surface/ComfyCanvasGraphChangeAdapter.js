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
/** Publish Comfy canvas graph transitions without polling graph state. */
/**
 * Own the reversible adapter around Comfy's graph-navigation boundary.
 *
 * Every host and SugarCubes transition passes through `setGraph`, so one
 * wrapper provides an event-driven source without scanning Cube definitions.
 */
export class ComfyCanvasGraphChangeAdapter {
    #canvas;
    #originalSetGraph;
    #wrappedSetGraph;
    #listeners = new Set();
    #disposed = false;
    /** Wrap one dynamic Comfy canvas after validating its navigation surface. */
    constructor(canvas) {
        if (typeof canvas.setGraph !== 'function') {
            throw new TypeError('Comfy canvas setGraph is unavailable.');
        }
        this.#canvas = canvas;
        this.#originalSetGraph = canvas.setGraph;
        this.#wrappedSetGraph = (graph, ...args) => this.#setGraph(graph, args);
        canvas.setGraph = this.#wrappedSetGraph;
    }
    /** Invoke the same wrapped boundary used by host navigation. */
    setGraph(graph) {
        this.#wrappedSetGraph(graph);
    }
    /** Subscribe one presentation collaborator to actual graph transitions. */
    subscribe(listener) {
        if (this.#disposed)
            return () => undefined;
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Restore Comfy's exact method when this adapter still owns the boundary. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        if (this.#canvas.setGraph === this.#wrappedSetGraph) {
            this.#canvas.setGraph = this.#originalSetGraph;
        }
        this.#listeners.clear();
    }
    /** Preserve Comfy's receiver and notify only after a successful transition. */
    #setGraph(graph, args) {
        const result = this.#originalSetGraph.call(this.#canvas, graph, ...args);
        for (const listener of this.#listeners)
            listener();
        return result;
    }
}
