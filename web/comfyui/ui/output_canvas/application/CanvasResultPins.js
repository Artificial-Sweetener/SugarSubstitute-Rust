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
/** Retain at most two lightweight final descriptors for a view's explicit comparison. */
/** Keep comparison revisions stable without retaining decoded pixels or workflow history. */
export class CanvasResultPins {
    #views = new Map();
    /** Replace the complete bounded pair through the selection authority. */
    set(view, pair) {
        this.#views.set(view, Object.freeze({ ...pair }));
    }
    /** Read the pinned revisions independently of replacement output arrivals. */
    read(view) {
        return this.#views.get(view) ?? null;
    }
    /** Release both descriptors when comparison or the view ends. */
    clear(view) {
        this.#views.delete(view);
    }
    /** Drop all retained descriptor references at integration teardown. */
    dispose() {
        this.#views.clear();
    }
}
