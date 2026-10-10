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
/** Coordinate refreshes for every Cube discovery catalog. */
/** Fan pack changes out to independent discovery consumers with isolated failures. */
export class CubeCatalogInvalidationCoordinator {
    #refreshPicker;
    #refreshLibrary;
    #logger;
    /** Bind the independent catalog refresh operations. */
    constructor(options) {
        this.#refreshPicker = options.refreshPicker;
        this.#refreshLibrary = options.refreshLibrary;
        this.#logger = options.logger;
    }
    /** Refresh every dependent catalog without letting one failure suppress another. */
    async invalidate() {
        const [picker, library] = await Promise.allSettled([
            this.#refreshPicker(),
            this.#refreshLibrary(),
        ]);
        if (picker.status === 'rejected') {
            this.#logger.warn('SugarCubes: failed to refresh the native picker catalog.', {
                error: picker.reason,
            });
        }
        if (library.status === 'rejected') {
            this.#logger.warn('SugarCubes: failed to refresh the library metadata catalog.', {
                error: library.reason,
            });
        }
    }
}
