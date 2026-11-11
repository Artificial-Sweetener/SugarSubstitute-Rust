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
/** Adapt graph-owned native Cube nodes to the save application boundary. */
import { listCubeNodeInstances } from './CubeNodeInstanceCatalog.js';
import { updateCubeNodeIdentityForIds, } from './CubeNodeIdentityWriter.js';
import { writeCubeNodeDocumentsForIds } from './CubeNodeDocumentWriter.js';
/** Isolate live Comfy node lookup and mutation from save orchestration. */
export class ComfyCubeSaveAdapter {
    #getCatalog;
    constructor({ getCatalog }) {
        this.#getCatalog = getCatalog;
    }
    /** Snapshot every graph-owned Cube node into typed save input. */
    listInstances() {
        return listCubeNodeInstances(this.#getCatalog()?.list() ?? []);
    }
    /** Apply save identity changes to exactly the addressed Cube instances. */
    updateIdentities(instanceIds, updates) {
        const catalog = this.#getCatalog();
        return catalog ? updateCubeNodeIdentityForIds(catalog, instanceIds, updates) : 0;
    }
    /** Persist one finalized portable document beside each addressed native definition. */
    updateDocuments(instanceIds, document, identity) {
        const catalog = this.#getCatalog();
        return catalog ? writeCubeNodeDocumentsForIds(catalog, instanceIds, document, identity) : 0;
    }
}
