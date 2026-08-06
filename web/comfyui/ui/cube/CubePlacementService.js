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
/** Insert constructed Cube nodes with Sugar-owned catalog and history behavior. */
/** Own root-graph insertion, catalog registration, and history boundaries. */
export class CubePlacementService {
    #construction;
    #graph;
    #catalog;
    #history;
    /** Bind detached construction to the normal Sugar import insertion policy. */
    constructor(options) {
        this.#construction = options.construction;
        this.#graph = options.graph;
        this.#catalog = options.catalog;
        this.#history = options.history;
    }
    /** Construct and insert one imported Cube definition. */
    place(payload, options = {}) {
        return this.#withHistory(true, () => this.#insert(this.#construction.construct(payload, options)));
    }
    /** Construct and insert one prebuilt native definition. */
    placeBuilt(request) {
        return this.#withHistory(request.recordHistory, () => this.#insert(this.#construction.constructBuilt(request)));
    }
    /** Insert one already-constructed node through normal Sugar placement ownership. */
    #insert(constructed) {
        this.#graph.add(constructed.node);
        this.#catalog.add(constructed.node);
        this.#history.setDirtyCanvas?.(true, true);
        return constructed;
    }
    /** Preserve one balanced host history boundary around a placement mutation. */
    #withHistory(recordHistory, operation) {
        if (recordHistory)
            this.#history.beforeChange?.();
        try {
            return operation();
        }
        finally {
            if (recordHistory)
                this.#history.afterChange?.();
        }
    }
}
