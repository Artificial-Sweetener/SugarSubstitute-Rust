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
/** Prevent Sugar-marked documents from crossing Comfy's Blueprint persistence boundary. */
import { isRecord } from '../types/common.js';
/** Guard the stable user-data write boundary while preserving ordinary Blueprint writes. */
export class CubeBlueprintPersistenceGuard {
    #api;
    #nativeStoreUserData;
    #installed = false;
    /** Validate the one host API method used by Comfy Blueprint persistence. */
    constructor(api) {
        if (!isRecord(api) || typeof api.storeUserData !== 'function') {
            throw new TypeError('Comfy user-data persistence is unavailable.');
        }
        this.#api = api;
        this.#nativeStoreUserData = this.#api.storeUserData;
    }
    /** Install one content-aware persistence guard for this extension lifetime. */
    install() {
        if (this.#installed)
            return;
        this.#api.storeUserData = (...args) => {
            if (isSubgraphBlueprintPath(args[0]) && containsSugarMarkers(args[1])) {
                throw new Error('SugarCubes are saved as versioned .cube documents and cannot be published as Subgraph Blueprints.');
            }
            return this.#nativeStoreUserData.apply(this.#api, args);
        };
        this.#installed = true;
    }
    /** Restore Comfy's native API for test isolation or extension teardown. */
    dispose() {
        if (!this.#installed)
            return;
        this.#api.storeUserData = this.#nativeStoreUserData;
        this.#installed = false;
    }
}
/** Recognize Comfy's stable user Blueprint storage namespace. */
function isSubgraphBlueprintPath(value) {
    return typeof value === 'string' && value.replaceAll('\\', '/').startsWith('subgraphs/');
}
/** Inspect serialized or object payloads without rendering or executing their contents. */
function containsSugarMarkers(value) {
    let serialized;
    try {
        serialized = typeof value === 'string' ? value : JSON.stringify(value);
    }
    catch {
        return false;
    }
    return (typeof serialized === 'string' &&
        (serialized.includes('"sugarcubes_kind"') || serialized.includes('"sugarcubes_cube"')));
}
