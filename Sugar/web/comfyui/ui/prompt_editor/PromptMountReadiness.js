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
/** Complete component construction from its first mounted native layout publication. */
export class PromptMountReadiness {
    #waiting = new Set();
    #presented = false;
    #retired = false;
    /** Await the component's normal layout publication without requesting another measurement. */
    wait() {
        if (this.#retired || this.#presented)
            return Promise.resolve();
        return new Promise((resolve) => this.#waiting.add(resolve));
    }
    /** Accept mounted native output without introducing a second geometry reader or sizing authority. */
    publish(width) {
        if (this.#retired || width <= 0)
            return;
        this.#presented = true;
        this.#release();
    }
    /** Release a retired presentation so its caller can discard an obsolete construction. */
    dispose() {
        this.#retired = true;
        this.#release();
    }
    /** Complete each construction waiter exactly once. */
    #release() {
        const waiting = [...this.#waiting];
        this.#waiting.clear();
        for (const resolve of waiting)
            resolve();
    }
}
