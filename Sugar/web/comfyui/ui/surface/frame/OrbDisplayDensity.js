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
/** Observe monitor and browser zoom density without polling or rebuilding frame controls. */
export class OrbDisplayDensity {
    window;
    changed;
    #query = null;
    #disposed = false;
    /** Notify the raster owner when its physical pixel destination changes. */
    constructor(window, changed) {
        this.window = window;
        this.changed = changed;
        this.#arm();
    }
    /** Re-arm the exact density query after each display change. */
    #changed = () => {
        this.#arm();
        this.changed();
    };
    /** Keep exactly one media listener tied to the current device scale. */
    #arm() {
        this.#query?.removeEventListener('change', this.#changed);
        if (!this.window?.matchMedia || this.#disposed)
            return;
        this.#query = this.window.matchMedia(`(resolution: ${String(this.window.devicePixelRatio || 1)}dppx)`);
        this.#query.addEventListener('change', this.#changed);
    }
    /** Release the density callback when the orb is disconnected. */
    dispose() {
        this.#disposed = true;
        this.#query?.removeEventListener('change', this.#changed);
        this.#query = null;
    }
}
