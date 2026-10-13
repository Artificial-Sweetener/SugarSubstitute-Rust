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
/** Own linked and independent lightweight inspection state within each canvas view. */
import { captureCanvasInspection, projectCanvasInspection, } from '../domain/CanvasInspection.js';
import { centeredCanvasCamera, } from '../domain/CanvasCamera.js';
/** Bound independent camera references to current workflow slots rather than accumulating output history. */
export class CanvasInspectionStore {
    #views = new Map();
    /** Reset replacement generations and prune slots removed from the current catalog. */
    synchronize(view, generation, slots) {
        let record = this.#views.get(view);
        if (!record || record.generation !== generation) {
            record = {
                generation,
                presentedRun: null,
                linked: true,
                shared: null,
                individual: new Map(),
            };
            this.#views.set(view, record);
        }
        const valid = new Set(slots.map(selectionKey));
        for (const slot of record.individual.keys())
            if (!valid.has(slot))
                record.individual.delete(slot);
    }
    /** Retire previous-Run cameras only when a newer Run's pixels are actually presented. */
    presentRun(view, runOrder) {
        const record = this.#require(view);
        if (record.presentedRun !== null && runOrder <= record.presentedRun)
            return false;
        record.presentedRun = runOrder;
        record.shared = null;
        record.individual.clear();
        return true;
    }
    /** Change link policy without overwriting saved independent slot cameras. */
    setLinked(view, linked, current) {
        const record = this.#require(view);
        if (record.linked === linked)
            return;
        if (current) {
            const inspection = captureCanvasInspection(current.camera, current.geometry);
            const key = selectionKey(current.selection);
            if (linked)
                record.shared = inspection;
            else if (!record.individual.has(key))
                record.individual.set(key, inspection);
        }
        record.linked = linked;
    }
    /** Save accepted inspection without copying a camera into another workflow or view. */
    capture(view, slot, camera, geometry) {
        const record = this.#require(view);
        const inspection = captureCanvasInspection(camera, geometry);
        if (record.linked)
            record.shared = inspection;
        else
            record.individual.set(selectionKey(slot), inspection);
    }
    /** Reproject normalized inspection against actual decoded dimensions and current density. */
    camera(view, slot, geometry) {
        const record = this.#require(view);
        const inspection = record.linked ? record.shared : record.individual.get(selectionKey(slot));
        return inspection
            ? projectCanvasInspection(inspection, geometry)
            : centeredCanvasCamera(geometry);
    }
    /** Release all image-relative state when its view closes. */
    remove(view) {
        this.#views.delete(view);
    }
    /** End this integration without keeping workflow inspection state alive. */
    dispose() {
        this.#views.clear();
    }
    /** Require lifecycle enrollment before accepting camera state. */
    #require(view) {
        const record = this.#views.get(view);
        if (!record)
            throw new Error('Canvas inspection view is unavailable.');
        return record;
    }
}
/** Key independent slots without retaining superseded execution result identities. */
function selectionKey(selection) {
    return JSON.stringify([selection.sourceId, selection.batchIndex]);
}
