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
/** Observe native card dimensions and coalesce responsive masonry reflows. */
/** Own browser resize observation for one Cube surface. */
export class NativeCardGeometryObserver {
    #window;
    #onGeometryChange;
    #observer;
    #animationFrame = null;
    /** Bind native ResizeObserver updates to one coalesced layout callback. */
    constructor(documentRef, onGeometryChange) {
        const windowRef = documentRef.defaultView;
        if (!windowRef)
            throw new Error('Cube card geometry requires a browser window.');
        this.#window = windowRef;
        this.#onGeometryChange = onGeometryChange;
        this.#observer =
            typeof windowRef.ResizeObserver === 'function'
                ? new windowRef.ResizeObserver(() => this.#schedule())
                : null;
    }
    /** Replace the exact native card cells whose dimensions drive masonry. */
    observe(cells) {
        this.#observer?.disconnect();
        for (const cell of cells)
            this.#observer?.observe(cell);
        this.#schedule();
    }
    /** Disconnect browser resources owned by this Cube surface. */
    dispose() {
        this.#observer?.disconnect();
        if (this.#animationFrame !== null) {
            this.#window.cancelAnimationFrame(this.#animationFrame);
            this.#animationFrame = null;
        }
    }
    /** Schedule one post-render measurement regardless of callback burst size. */
    #schedule() {
        if (this.#animationFrame !== null)
            return;
        this.#animationFrame = this.#window.requestAnimationFrame(() => {
            this.#animationFrame = null;
            this.#onGeometryChange();
        });
    }
}
