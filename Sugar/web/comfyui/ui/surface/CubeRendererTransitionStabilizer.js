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
/** Reconcile Cube presentation across Comfy's bounded renderer transition window. */
/** Coalesce lifecycle events into a short, event-driven renderer stabilization pass. */
export class CubeRendererTransitionStabilizer {
    #requestFrame;
    #cancelFrame;
    #frameCount;
    #pendingFrame = null;
    #generation = 0;
    /** Bind the host scheduler without creating an idle polling loop. */
    constructor(options) {
        this.#requestFrame = options.requestFrame;
        this.#cancelFrame = options.cancelFrame;
        this.#frameCount = Math.max(1, Math.trunc(options.frameCount ?? 4));
    }
    /** Recheck presentation for the few frames in which Comfy replaces renderer hooks. */
    run(reconcile) {
        this.#generation += 1;
        const generation = this.#generation;
        if (this.#pendingFrame !== null)
            this.#cancelFrame(this.#pendingFrame);
        this.#pendingFrame = null;
        this.#schedule(reconcile, generation, this.#frameCount);
    }
    /** Cancel the one bounded transition pass, if active. */
    dispose() {
        this.#generation += 1;
        if (this.#pendingFrame !== null)
            this.#cancelFrame(this.#pendingFrame);
        this.#pendingFrame = null;
    }
    /** Schedule one remaining transition frame. */
    #schedule(reconcile, generation, remaining) {
        this.#pendingFrame = this.#requestFrame(() => {
            this.#pendingFrame = null;
            if (generation !== this.#generation)
                return;
            reconcile();
            if (remaining > 1)
                this.#schedule(reconcile, generation, remaining - 1);
        });
    }
}
