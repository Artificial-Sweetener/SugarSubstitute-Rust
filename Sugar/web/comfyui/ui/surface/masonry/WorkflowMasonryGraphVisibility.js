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
/** Keep graph layers suppressed across workflow replacement and restore native interaction exactly. */
import { workflowMasonryGraphLayers } from './WorkflowMasonryGraphGeometry.js';
/** Own graph visibility independently from the lifetime of borrowed masonry cards. */
export class WorkflowMasonryGraphVisibility {
    getContainer;
    #inert = new Map();
    #hidden = new Set();
    /** Resolve the current allocation even before a destination masonry surface exists. */
    constructor(getContainer) {
        this.getContainer = getContainer;
    }
    /** Suppress existing and replacement graph layers without removing their measurable layout. */
    setHidden(hidden) {
        this.setInert(hidden);
        if (!hidden) {
            for (const element of this.#hidden)
                delete element.dataset.sugarcubesMasonry;
            this.#hidden.clear();
            return;
        }
        const container = this.getContainer();
        if (!container)
            return;
        const root = container.closest('#graph-canvas-container');
        for (const element of new Set([container, root])) {
            if (!element)
                continue;
            this.#hidden.add(element);
            if (element.dataset.sugarcubesMasonry !== 'true')
                element.dataset.sugarcubesMasonry = 'true';
        }
    }
    /** Block native input during card travel while preserving each layer's prior inert state. */
    setInert(inert) {
        if (!inert) {
            for (const [element, previous] of this.#inert)
                element.inert = previous;
            this.#inert.clear();
            return;
        }
        const container = this.getContainer();
        if (!container)
            return;
        for (const layer of workflowMasonryGraphLayers(container)) {
            if (!this.#inert.has(layer))
                this.#inert.set(layer, layer.inert);
            if (!layer.inert)
                layer.inert = true;
        }
    }
}
