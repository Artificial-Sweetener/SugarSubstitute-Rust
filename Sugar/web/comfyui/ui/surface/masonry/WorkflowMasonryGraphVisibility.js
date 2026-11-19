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
import { MASONRY_HIDES_ATTRIBUTE } from './WorkflowMasonryStyles.js';
import { SurfaceInputHold } from '../motion/SurfaceInputHold.js';
/** Own graph visibility independently from the lifetime of borrowed masonry cards. */
export class WorkflowMasonryGraphVisibility {
    getContainer;
    #inert = new Map();
    #held = new Map();
    #hidden = new Set();
    #layers = new Set();
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
            for (const layer of this.#layers)
                if (layer.getAttribute(MASONRY_HIDES_ATTRIBUTE) === 'graph')
                    layer.removeAttribute(MASONRY_HIDES_ATTRIBUTE);
            this.#layers.clear();
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
        // Each layer carries its own mark, so hiding what is inside it restyles that layer alone.
        for (const layer of workflowMasonryGraphLayers(container)) {
            this.#layers.add(layer);
            if (layer.getAttribute(MASONRY_HIDES_ATTRIBUTE) !== 'graph')
                layer.setAttribute(MASONRY_HIDES_ATTRIBUTE, 'graph');
        }
    }
    /** Block native input during card travel while preserving each layer's prior inert state. */
    setInert(inert) {
        for (const hold of this.#held.values())
            hold.release();
        this.#held.clear();
        if (!inert) {
            for (const [element, previous] of this.#inert)
                if (element.inert !== previous)
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
    /**
     * Keep input out of the layers by a hold instead of `inert`, until `setInert` next runs.
     *
     * Called before borrowed cards return to a layer: `inert` reaches every descendant's style,
     * so clearing it once the cards are back restyles every one of them.
     */
    holdInput() {
        for (const [layer, previous] of this.#inert) {
            if (previous || this.#held.has(layer))
                continue;
            if (layer.inert)
                layer.inert = false;
            const hold = new SurfaceInputHold();
            hold.hold(layer);
            this.#held.set(layer, hold);
        }
    }
}
