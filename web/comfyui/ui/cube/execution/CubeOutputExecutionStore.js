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
/** Retain runtime results emitted by prompt-only Cube output marker sinks. */
import { isCubeOutputExecutionId } from './CubeOutputExecutionIdentity.js';
/** Own Cube output results that Comfy cannot associate with a graph node. */
export class CubeOutputExecutionStore {
    #outputs = new Map();
    #listeners = new Set();
    /** Return the number of currently retained Cube output results. */
    get size() {
        return this.#outputs.size;
    }
    /** Retain one validated prompt-only Cube output result. */
    retain(executionId, output) {
        if (!isCubeOutputExecutionId(executionId))
            return false;
        this.#outputs.set(executionId, output);
        this.#notify();
        return true;
    }
    /** Read the last result for one ordered Cube output boundary. */
    read(executionId) {
        return this.#outputs.get(executionId);
    }
    /** Discard results when Comfy replaces the root workflow. */
    clear() {
        if (this.#outputs.size === 0)
            return;
        this.#outputs.clear();
        this.#notify();
    }
    /** Observe coherent output changes without polling Comfy's media maps. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Wake preview presentation only after retained output state changes. */
    #notify() {
        for (const listener of this.#listeners)
            listener();
    }
}
