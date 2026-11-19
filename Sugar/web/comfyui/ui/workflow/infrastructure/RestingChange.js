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
/** Report a burst of changes once, after it has stopped. */
/**
 * Call `rested` once a run of `changed` calls has been quiet for `quietMs`.
 *
 * Panning a canvas or travelling between two views changes what a person is looking at on every
 * frame. Whoever records where they are looking needs the place they stop, not each place on the
 * way.
 */
export class RestingChange {
    timers;
    quietMs;
    rested;
    #timer = null;
    constructor(timers, quietMs, rested) {
        this.timers = timers;
        this.quietMs = quietMs;
        this.rested = rested;
    }
    /** Note one change; the report waits until changes stop. */
    changed() {
        if (this.#timer !== null)
            this.timers.clearTimeout(this.#timer);
        this.#timer = this.timers.setTimeout(() => {
            this.#timer = null;
            this.rested();
        }, this.quietMs);
    }
    /** Forget a pending report, for an owner that has just read the current state itself. */
    settle() {
        if (this.#timer === null)
            return;
        this.timers.clearTimeout(this.#timer);
        this.#timer = null;
    }
}
