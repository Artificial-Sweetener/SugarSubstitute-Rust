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
/** Hold the names of the files a host says are being dragged over its window. */
/**
 * The files of the drag now over the window, by file name, as a host that can see them reports.
 *
 * A page is told that a drag carries files but not what they are called until the drop. Null means
 * no host has named a drag: either none is over the window, or the page runs where nothing can
 * name one.
 */
export class DraggedFiles {
    #names = null;
    #listeners = new Set();
    /** The dragged files' names in the order the drag carries them, or null. */
    get names() {
        return this.#names;
    }
    /** Record the drag the host reports, or its end with null. */
    set(names) {
        if (names === null && this.#names === null)
            return;
        this.#names = names;
        for (const listener of [...this.#listeners])
            listener();
    }
    /** Hear about every change until the returned function is called. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
}
