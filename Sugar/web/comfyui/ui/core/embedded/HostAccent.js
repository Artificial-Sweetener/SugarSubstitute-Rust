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
/**
 * Own the connected native host's accent for this document, such as the operating-system
 * accent SugarSubstitute reads. It is offered under every host preference policy without
 * changing the user's saved browser preferences.
 */
/** Publish only actual host accent changes for this frontend lifetime. */
export class HostAccent {
    #value = null;
    #listeners = new Set();
    /** Read the host's schema-validated accent, or null when the host offers none. */
    read() {
        return this.#value;
    }
    /** Observe host accent changes. */
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    /** Adopt or withdraw the host accent; persisted preferences are never written. */
    set(value) {
        if (value === this.#value)
            return;
        this.#value = value;
        for (const changed of this.#listeners)
            changed();
    }
}
