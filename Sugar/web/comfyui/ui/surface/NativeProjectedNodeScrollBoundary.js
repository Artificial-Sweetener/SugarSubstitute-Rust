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
/** Isolate projected card wheel input before Comfy's native node forwarding. */
export class NativeProjectedNodeScrollBoundary {
    #bindings = new Map();
    #root = null;
    /** Preserve child control handlers and native scrolling while shielding graph zoom. */
    reconcile(root, controls) {
        this.dispose();
        this.#root = root;
        if (!root)
            return;
        root.addEventListener('wheel', this.#stopRootWheel, { capture: true, passive: true });
        const controlSet = new Set(controls);
        const boundaries = new Set([
            ...controls,
            ...Array.from(root.children).filter((element) => element instanceof HTMLElement),
        ]);
        for (const boundary of boundaries) {
            const stop = (event) => {
                if (controlSet.has(boundary) || root.closest('.sugarcubes-workflow-masonry'))
                    event.stopPropagation();
            };
            boundary.addEventListener('wheel', stop, { passive: true });
            this.#bindings.set(boundary, stop);
        }
    }
    /** Release only owned listeners when Vue replaces or unmounts a card. */
    dispose() {
        this.#root?.removeEventListener('wheel', this.#stopRootWheel, true);
        this.#root = null;
        for (const [element, listener] of this.#bindings)
            element.removeEventListener('wheel', listener);
        this.#bindings.clear();
    }
    /** Shield bare native card chrome without blocking descendants' widget handlers. */
    #stopRootWheel = (event) => {
        if (event.target === this.#root && this.#root?.closest('.sugarcubes-workflow-masonry'))
            event.stopImmediatePropagation();
    };
}
