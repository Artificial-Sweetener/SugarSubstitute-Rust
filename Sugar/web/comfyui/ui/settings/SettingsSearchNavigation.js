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
/** Preserve the user's settings page while Comfy owns global search and result rendering. */
import { activateSettingsCategory, readSettingsCategory } from './ComfySettingsNavigation.js';
/** Remember the page preceding a search while delegating filtering and navigation to Comfy. */
export class SettingsSearchNavigation {
    root;
    input;
    #returnTo = null;
    #searching = false;
    /** Observe search intent before native filtering clears the active category. */
    constructor(root, input) {
        this.root = root;
        this.input = input;
        input.addEventListener('input', this.#input, true);
        root.addEventListener('click', this.#navigate, true);
    }
    /** Restore the accepted page only after Comfy has completed its empty-query transition. */
    reconcile() {
        const selected = readSettingsCategory(this.root);
        if (!this.#searching || this.input.value.trim() || !selected)
            return false;
        this.#searching = false;
        const destination = this.#returnTo;
        this.#returnTo = null;
        return (!!destination &&
            destination !== selected.id &&
            activateSettingsCategory(this.root, destination));
    }
    /** Release presentation navigation without changing the active native query. */
    dispose() {
        this.input.removeEventListener('input', this.#input, true);
        this.root.removeEventListener('click', this.#navigate, true);
    }
    /** Remember the selected native page once at the beginning of a search session. */
    #input = () => {
        if (!this.input.value.trim() || this.#searching)
            return;
        this.#returnTo = readSettingsCategory(this.root)?.id ?? null;
        this.#searching = true;
    };
    /** Respect explicit category selection made while browsing filtered results. */
    #navigate = (event) => {
        const entry = event.target instanceof Element ? event.target.closest('[data-nav-id]') : null;
        if (this.#searching && entry)
            this.#returnTo = entry.dataset.navId ?? null;
    };
}
