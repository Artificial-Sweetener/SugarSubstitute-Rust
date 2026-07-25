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
/** Refresh proximity geometry after native Nodes 2.0 pointer movement. */
/** Own the DOM movement signal that LiteGraph canvas callbacks do not receive. */
export class ProximityPointerMoveTracker {
    #proximity;
    #attached = new WeakSet();
    /** Bind the renderer-neutral proximity preview owner. */
    constructor(proximity) {
        this.#proximity = proximity;
    }
    /** Attach one idempotent movement listener to the active graph surface. */
    attach(element, surface) {
        if (this.#attached.has(element))
            return;
        const refresh = () => {
            if (this.#proximity.isOverlayEnabled()) {
                this.#proximity.schedulePreview({ graph: surface.graph });
            }
        };
        element.addEventListener('pointermove', refresh, true);
        element.addEventListener('mousemove', refresh, true);
        this.#attached.add(element);
    }
}
