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
/** Bridge a native host's settled cameras to the view's shared inspection owner. */
import { canvasInspectionSlotKey, } from '../domain/CanvasInspection.js';
/** The schema's inclusive span ceiling: one image per physical pixel at the smallest scale. */
const MAXIMUM_SPAN = 1_000_000_000;
/**
 * Record and restore native cameras with the same semantics as the browser camera controller.
 *
 * Settled viewport states the presentation already reads after a selection, command or gesture
 * are recorded per view; selections restore them immediately, a newer Run starts at Fit and
 * previews are never recorded. Link policy follows the shared view state.
 */
export class NativeCanvasInspection {
    options;
    #linked = true;
    #current = null;
    #link = null;
    /** Bind one view's inspection owner and the presentation's camera command stream. */
    constructor(options) {
        this.options = options;
    }
    /**
     * Choose the camera a final selection restores: Fit for a newer Run, the presented camera
     * across comparison partners, and otherwise the view's saved inspection for that slot.
     */
    restore(item, compared, slot = itemSelection(item)) {
        const { view, inspections } = this.options;
        if (!this.options.enabled ||
            inspections.retires(view, item.provenance.runOrder, slot.scene?.[0] ?? null))
            return null;
        const saved = compared && this.#current ? this.#current.inspection : inspections.inspection(view, slot);
        return canvasSurfaceInspection(saved);
    }
    /** Record the settled camera of the shown final; previews and overview grids keep theirs. */
    observe(plan, viewport, preview) {
        const item = plan.primary?.item;
        if (!this.options.enabled ||
            !viewport ||
            preview ||
            plan.view.mode === 'overview' ||
            !item ||
            item.mediaKind !== 'image' ||
            (item.dimensions !== null &&
                (item.dimensions.width !== viewport.dimensions.width ||
                    item.dimensions.height !== viewport.dimensions.height)))
            return;
        const inspection = nativeCanvasViewportInspection(viewport);
        if (!inspection)
            return;
        const { view, inspections } = this.options;
        const selection = plan.primary?.selection ?? itemSelection(item);
        inspections.presentRun(view, item.provenance.runOrder, selection.scene?.[0] ?? null);
        inspections.record(view, selection, inspection);
        this.#current = Object.freeze({ selection, inspection });
    }
    /**
     * Reconcile the view's link preference like the browser camera, then show the shown slot's
     * resulting inspection, or Fit when that slot has none.
     */
    link(plan) {
        const linked = plan.view.linked;
        if (!this.options.enabled || linked === this.#linked)
            return;
        this.#linked = linked;
        const shown = plan.primary?.selection;
        const current = shown &&
            this.#current &&
            canvasInspectionSlotKey(this.#current.selection) === canvasInspectionSlotKey(shown)
            ? this.#current
            : null;
        const { view, inspections } = this.options;
        inspections.setLinked(view, linked, current);
        if (!current)
            return;
        const inspection = canvasSurfaceInspection(inspections.inspection(view, current.selection));
        this.#link?.abort();
        const link = new AbortController();
        this.#link = link;
        void this.options.viewport(inspection ? { type: 'inspect', inspection } : { type: 'fit' }, {
            signal: link.signal,
            current: () => !link.signal.aborted && this.options.accepts(),
        });
    }
    /** Withdraw a pending link camera; saved inspections outlive this attachment. */
    dispose() {
        this.#link?.abort();
        this.#link = null;
    }
}
/** Translate a settled native camera into the shared image-relative inspection. */
export function nativeCanvasViewportInspection(viewport) {
    const { center, dimensions, physicalScale } = viewport;
    if (!center)
        return null;
    return Object.freeze({
        mode: viewport.mode,
        region: Object.freeze({
            centerX: center.x,
            centerY: center.y,
            spanX: 1 / (dimensions.width * physicalScale),
            spanY: 1 / (dimensions.height * physicalScale),
        }),
    });
}
/**
 * Express a saved inspection in the wire's bounds, or null where the host's Fit is the camera.
 * Relative centers remain unbounded; a span outside the schema bounds cannot be restored.
 */
export function canvasSurfaceInspection(inspection) {
    if (!inspection || inspection.mode === 'fit')
        return null;
    const { centerX, centerY, spanX, spanY } = inspection.region;
    if (![centerX, centerY].every(Number.isFinite) ||
        ![spanX, spanY].every((span) => Number.isFinite(span) && span > 0 && span <= MAXIMUM_SPAN))
        throw new RangeError('Saved canvas inspection cannot be represented by the native host.');
    return Object.freeze({
        center: Object.freeze({ x: centerX, y: centerY }),
        span: Object.freeze({ x: spanX, y: spanY }),
    });
}
/** Identify the shared view slot that a final output item occupies. */
function itemSelection(item) {
    return Object.freeze({ sourceId: item.provenance.sourceId, batchIndex: item.batchIndex });
}
