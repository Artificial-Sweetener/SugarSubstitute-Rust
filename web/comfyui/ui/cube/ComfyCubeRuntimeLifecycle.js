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
/** Own the graph-bound Cube runtime across Comfy workflow configurations. */
/** Recreate host-bound collaborators after Comfy resets graph UI state. */
export class ComfyCubeRuntimeLifecycle {
    #create;
    #runtime = null;
    /** Bind the focused runtime factory without resolving host state eagerly. */
    constructor(create) {
        this.#create = create;
    }
    /** Resolve the runtime for the currently configured Comfy graph. */
    require() {
        this.#runtime ??= this.#create();
        return this.#runtime;
    }
    /** Return the current graph runtime without creating host-bound collaborators. */
    current() {
        return this.#runtime;
    }
    /** Release graph-bound presentation and adapters before graph replacement. */
    reset() {
        this.#runtime?.dispose();
        this.#runtime = null;
    }
}
