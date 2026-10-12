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
/** Observe one image viewport and its display density without polling geometry. */
/** Coalesce settled resize/density notifications within the attachment's document. */
export class BrowserCanvasGeometry {
    element;
    frames;
    changed;
    #resize;
    #stops = [];
    #media = null;
    #frame = null;
    #disposed = false;
    /** Bind explicit document/window geometry and release observer/listener ownership together. */
    constructor(element, frames, changed) {
        this.element = element;
        this.frames = frames;
        this.changed = changed;
        const window = element.ownerDocument.defaultView;
        if (!window)
            throw new Error('Canvas document window is unavailable.');
        const Observer = window.ResizeObserver;
        this.#resize = Observer ? new Observer(() => this.#schedule()) : null;
        this.#resize?.observe(element);
        const schedule = () => this.#schedule();
        window.addEventListener('resize', schedule);
        window.visualViewport?.addEventListener('resize', schedule);
        element.addEventListener('focusin', schedule);
        element.addEventListener('focusout', schedule);
        this.#stops.push(() => window.removeEventListener('resize', schedule), () => window.visualViewport?.removeEventListener('resize', schedule), () => element.removeEventListener('focusin', schedule), () => element.removeEventListener('focusout', schedule));
        this.#density();
    }
    /** Read CSS allocation and display density without traversing the workflow graph. */
    read() {
        const bounds = this.element.getBoundingClientRect();
        const document = this.element.ownerDocument;
        return Object.freeze({
            coordinateSpace: 'client',
            units: 'css-pixels',
            origin: Object.freeze({ x: bounds.left, y: bounds.top }),
            viewport: Object.freeze({ width: bounds.width, height: bounds.height }),
            clip: Object.freeze({
                x: bounds.left,
                y: bounds.top,
                width: bounds.width,
                height: bounds.height,
            }),
            physicalPixelsPerCssPixel: document.defaultView?.devicePixelRatio || 1,
            visible: document.visibilityState !== 'hidden' && bounds.width > 0 && bounds.height > 0,
            focused: this.element.contains(document.activeElement),
        });
    }
    /** Stop geometry delivery before the viewport is detached. */
    dispose() {
        if (this.#disposed)
            return;
        this.#disposed = true;
        this.#resize?.disconnect();
        this.#frame?.();
        this.#frame = null;
        this.#media?.();
        this.#media = null;
        for (const stop of this.#stops.splice(0))
            stop();
    }
    /** Re-arm a density notification only when monitor/page density changes. */
    #density() {
        const window = this.element.ownerDocument.defaultView;
        this.#media?.();
        if (!window?.matchMedia || this.#disposed)
            return;
        const query = window.matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
        const changed = () => {
            this.#density();
            this.#schedule();
        };
        query.addEventListener('change', changed);
        this.#media = () => query.removeEventListener('change', changed);
    }
    /** Schedule one measurement for a burst of geometry events. */
    #schedule() {
        if (this.#frame || this.#disposed)
            return;
        this.#frame = this.frames.request(() => {
            this.#frame = null;
            if (!this.#disposed)
                this.changed(this.read());
        });
    }
}
