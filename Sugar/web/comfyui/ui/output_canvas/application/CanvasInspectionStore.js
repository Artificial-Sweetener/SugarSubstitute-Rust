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
import { canvasInspectionSlotKey as selectionKey, } from '../domain/CanvasInspection.js';
import { captureCanvasInspection, projectCanvasInspection, } from '../domain/CanvasInspection.js';
import { centeredCanvasCamera, } from '../domain/CanvasCamera.js';
/** Bound independent camera references to current workflow slots rather than accumulating output history. */
export class CanvasInspectionStore {
    #views = new Map();
    #listeners = new Set();
    /** Reset replacement generations and prune slots removed from the current catalog. */
    synchronize(view, generation, slots) {
        let record = this.#views.get(view);
        if (!record || record.generation !== generation) {
            record = {
                generation,
                presentedRun: null,
                presentedGroup: null,
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
    presentRun(view, runOrder, group = null) {
        const record = this.#require(view);
        if (!this.retires(view, runOrder, group)) {
            if (record.presentedRun !== null && runOrder > record.presentedRun)
                record.presentedRun = runOrder;
            return false;
        }
        record.presentedRun = runOrder;
        record.presentedGroup = group;
        record.shared = null;
        record.individual.clear();
        this.#notify(view);
        return true;
    }
    /** Report whether presenting this Run's final would retire saved cameras, without retiring them. */
    retires(view, runOrder, group = null) {
        const record = this.#require(view);
        return (record.presentedRun === null ||
            (runOrder > record.presentedRun && (group === null || group !== record.presentedGroup)));
    }
    /** Change link policy without overwriting saved independent slot cameras. */
    setLinked(view, linked, current) {
        const record = this.#require(view);
        if (record.linked === linked)
            return;
        if (current) {
            const key = selectionKey(current.selection);
            if (linked)
                record.shared = current.inspection;
            else if (!record.individual.has(key))
                record.individual.set(key, freezeSample(current));
        }
        record.linked = linked;
        this.#notify(view);
    }
    /** Save final inspection without allowing preview presentation to replace it. */
    capture(view, slot, camera, geometry, previewLane) {
        if (previewLane !== null) {
            this.#require(view);
            return;
        }
        this.record(view, slot, captureCanvasInspection(camera, geometry));
    }
    /** Save one final slot's settled inspection under the view's current link policy. */
    record(view, slot, inspection) {
        const record = this.#require(view);
        const next = freezeInspection(inspection);
        const previous = record.linked
            ? record.shared
            : record.individual.get(selectionKey(slot))?.inspection;
        if (sameInspection(previous, next))
            return;
        if (record.linked)
            record.shared = next;
        else
            record.individual.set(selectionKey(slot), freezeSample({ selection: slot, inspection: next }));
        this.#notify(view);
    }
    /** Observe settled final-camera changes so checkpoint capture does not depend on DOM or frame polling. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Read the inspection a final in this slot restores, or null when it starts at Fit. */
    inspection(view, slot) {
        const record = this.#require(view);
        return ((record.linked ? record.shared : record.individual.get(selectionKey(slot))?.inspection) ??
            null);
    }
    /** Capture both link lanes; choosing linked mode never discards retained independent inspections. */
    snapshot(view) {
        const record = this.#require(view);
        return Object.freeze({
            linked: record.linked,
            shared: record.shared ? freezeInspection(record.shared) : null,
            individual: Object.freeze([...record.individual.values()].map(freezeSample)),
            presentedRun: record.presentedRun,
            presentedGroup: record.presentedGroup,
        });
    }
    /** Restore final inspection before presentation so the first imported Run cannot erase it. */
    restore(view, generation, snapshot, slots) {
        const valid = new Set(slots.map(selectionKey));
        const individual = new Map();
        for (const sample of snapshot.individual) {
            const key = selectionKey(sample.selection);
            if (!valid.has(key) || individual.has(key))
                throw new TypeError('Restored independent canvas inspection slot is invalid.');
            individual.set(key, freezeSample(sample));
        }
        this.#views.set(view, {
            generation,
            linked: snapshot.linked,
            shared: snapshot.shared ? freezeInspection(snapshot.shared) : null,
            individual,
            presentedRun: snapshot.presentedRun,
            presentedGroup: snapshot.presentedGroup ?? null,
        });
        this.#notify(view);
    }
    /** Fit new preview compositions independently of existing linked final inspection. */
    camera(view, slot, geometry, previewLane) {
        const inspection = this.inspection(view, slot);
        if (previewLane !== null)
            return centeredCanvasCamera(geometry);
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
        this.#listeners.clear();
    }
    /** Require lifecycle enrollment before accepting camera state. */
    #require(view) {
        const record = this.#views.get(view);
        if (!record)
            throw new Error('Canvas inspection view is unavailable.');
        return record;
    }
    /** Notify explicit view owners without discovering graph or presentation surfaces. */
    #notify(view) {
        for (const listener of this.#listeners)
            listener(view);
    }
}
/** Freeze finite image-relative camera values without constraining centers to the image rectangle. */
function freezeInspection(inspection) {
    const region = inspection.region;
    if (!['fit', 'native', 'custom'].includes(inspection.mode) ||
        ![region.centerX, region.centerY, region.spanX, region.spanY].every(Number.isFinite) ||
        region.spanX <= 0 ||
        region.spanY <= 0)
        throw new TypeError('Restored canvas inspection is invalid.');
    return Object.freeze({ mode: inspection.mode, region: Object.freeze({ ...region }) });
}
/** Copy selection and camera facts so snapshots cannot mutate retained owner state. */
function freezeSample(sample) {
    return Object.freeze({
        selection: Object.freeze({
            ...sample.selection,
            ...(sample.selection.scene
                ? { scene: Object.freeze([...sample.selection.scene]) }
                : {}),
        }),
        inspection: freezeInspection(sample.inspection),
    });
}
/** Coalesce repeated renderer echoes of the same image-relative final inspection. */
function sameInspection(previous, next) {
    return (!!previous &&
        previous.mode === next.mode &&
        previous.region.centerX === next.region.centerX &&
        previous.region.centerY === next.region.centerY &&
        previous.region.spanX === next.region.spanX &&
        previous.region.spanY === next.region.spanY);
}
