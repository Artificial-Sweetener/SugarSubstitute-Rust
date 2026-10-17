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
/** Share accepted appearance facts without introducing another settings or palette owner. */
/** Publish coherent presentation facts only when their accepted values change. */
export class AppearanceState {
    #listeners = new Set();
    #appearance = Object.freeze({ fluent: false, dark: true });
    /** Expose an immutable accepted snapshot to isolated owned surfaces. */
    get appearance() {
        return this.#appearance;
    }
    /** Retain one observer until its owning surface releases the subscription. */
    subscribeAppearance(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Publish only facts supplied by the existing appearance owner. */
    update(fluent, dark) {
        if (fluent === this.#appearance.fluent && dark === this.#appearance.dark)
            return;
        this.#appearance = Object.freeze({ fluent, dark });
        for (const listener of this.#listeners)
            listener();
    }
    /** Release surface subscriptions with the host appearance lifetime. */
    dispose() {
        this.#listeners.clear();
    }
}
