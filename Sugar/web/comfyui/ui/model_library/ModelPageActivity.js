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
/** Own reversible model UI activity across cached-page suspension and final document teardown. */
export class ModelPageActivity {
    #window;
    #change;
    #discard;
    #observing = false;
    #active = false;
    /** Bind lifecycle transitions without owning model state, network jobs or renderer mounts. */
    constructor(window, change, discard) {
        this.#window = window;
        this.#change = change;
        this.#discard = discard;
    }
    /** Expose whether the current document may perform model UI work. */
    get active() {
        return this.#active;
    }
    /** Observe a live page once, retaining restoration listeners while the browser caches it. */
    start() {
        if (this.#observing)
            return;
        this.#observing = true;
        this.#window.addEventListener('pagehide', this.#hide);
        this.#window.addEventListener('pageshow', this.#show);
        this.#setActive(true);
    }
    /** Release all page listeners and stop scoped model UI work. */
    dispose() {
        if (!this.#observing)
            return;
        this.#observing = false;
        this.#window.removeEventListener('pagehide', this.#hide);
        this.#window.removeEventListener('pageshow', this.#show);
        this.#setActive(false);
    }
    /** Publish only genuine activity changes so duplicate browser events cannot remount controls. */
    #setActive(active) {
        if (this.#active === active)
            return;
        this.#active = active;
        this.#change(active);
    }
    /** Suspend cached pages; discard owners only when the document will not be restored. */
    #hide = (event) => {
        if (event.persisted)
            this.#setActive(false);
        else {
            this.dispose();
            this.#discard();
        }
    };
    /** Resume the same owner when the browser restores its cached document. */
    #show = (event) => {
        if (event.persisted)
            this.#setActive(true);
    };
}
