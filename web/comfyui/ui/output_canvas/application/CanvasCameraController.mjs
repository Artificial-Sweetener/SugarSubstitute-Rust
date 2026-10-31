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
/** Coordinate one attachment's displayed camera with durable view-relative inspection. */
import { centeredCanvasCamera, doubleClickCanvasCamera, panCanvasCamera, zoomCanvasCamera, canvasWheelFactor, } from '../domain/CanvasCamera.js';
import { resizeCanvasCamera, captureCanvasInspection, projectCanvasInspection, } from '../domain/CanvasInspection.js';
import { CanvasCameraTransition } from './CanvasCameraTransition.mjs';
/** Keep browser input and DOM transforms outside authoritative camera and inspection transitions. */
export class CanvasCameraController {
    options;
    #transition;
    #target = null;
    #disposed = false;
    #comparison = false;
    /** Bind an enrolled view's inspection owner and this attachment's cancellable frame clock. */
    constructor(options) {
        this.options = options;
        this.#transition = new CanvasCameraTransition(options.clock, (camera) => {
            const target = this.#target;
            if (!target || this.#disposed)
                return;
            options.inspections.capture(options.view, target.selection, camera, target.geometry, target.previewLane);
            options.present(Object.freeze({ ...target, camera }));
        });
    }
    /** Read the displayed transform so interrupted input rebases from visible pixels. */
    read() {
        const camera = this.#transition.read();
        return this.#target && camera ? Object.freeze({ ...this.#target, camera }) : null;
    }
    /** Only decoded final images admit user camera changes. */
    navigable() {
        return !this.#disposed && this.#target?.previewLane === null;
    }
    /** Fit fresh preview compositions and new Runs; preserve inspection within one asset's updates. */
    show(selection, geometry, runOrder, previewLane) {
        if (this.#disposed)
            return;
        const freshRun = previewLane === null && this.options.inspections.presentRun(this.options.view, runOrder);
        const displayed = this.read();
        const previous = freshRun || displayed?.previewLane !== previewLane ? null : displayed;
        const sameSlot = previous?.selection.sourceId === selection.sourceId &&
            previous.selection.batchIndex === selection.batchIndex;
        const sameImage = previous?.geometry.image.width === geometry.image.width &&
            previous.geometry.image.height === geometry.image.height;
        if (sameSlot &&
            sameImage &&
            previous.geometry.viewport.width === geometry.viewport.width &&
            previous.geometry.viewport.height === geometry.viewport.height &&
            previous.geometry.density === geometry.density)
            return;
        const camera = previewLane !== null
            ? centeredCanvasCamera(geometry)
            : sameSlot && sameImage
                ? resizeCanvasCamera(previous.camera, previous.geometry, geometry)
                : this.#comparison && previous
                    ? projectCanvasInspection(captureCanvasInspection(previous.camera, previous.geometry), geometry)
                    : this.options.inspections.camera(this.options.view, selection, geometry, previewLane);
        this.#target = { selection: Object.freeze({ ...selection }), geometry, previewLane };
        this.#transition.set(camera);
    }
    /** Preserve the common reveal region when changing comparison partners, independently of solo linking. */
    comparison(enabled) {
        this.#comparison = enabled;
    }
    /** Reconcile a link preference through the existing inspection owner, preserving independent slot state. */
    link(enabled) {
        if (this.#disposed)
            return;
        const current = this.read();
        this.options.inspections.setLinked(this.options.view, enabled, current && current.previewLane === null
            ? {
                selection: current.selection,
                inspection: captureCanvasInspection(current.camera, current.geometry),
            }
            : null);
        const target = this.#target;
        if (target)
            this.#transition.set(this.options.inspections.camera(this.options.view, target.selection, target.geometry, target.previewLane));
    }
    /** Fit the displayed image without changing content following. */
    fit() {
        return this.#move((target) => centeredCanvasCamera(target.geometry), null);
    }
    /** Center one decoded source pixel per physical display pixel. */
    native() {
        return this.#move((target) => centeredCanvasCamera(target.geometry, 'native'), null);
    }
    /** Zoom around a pointer/centroid or viewport center using the sole snap owner. */
    zoom(factor, anchor = null, wheel = false) {
        const current = this.read();
        if (!current)
            return false;
        const point = anchor ?? center(current.geometry);
        return this.#move((target) => zoomCanvasCamera(target.camera, factor, point, target.geometry), point, wheel);
    }
    /** Normalize wheel units without truncating fractional trackpad movement. */
    wheel(delta, units, anchor) {
        const target = this.read();
        return target
            ? this.zoom(canvasWheelFactor(delta, units, target.geometry.viewport.height), anchor, true)
            : false;
    }
    /** Pan directly without inertia or tweening continuous drag input. */
    pan(delta) {
        const target = this.read();
        if (!target || !this.navigable())
            return false;
        this.#transition.set(panCanvasCamera(target.camera, delta, target.geometry));
        return true;
    }
    /** Combine centroid-anchored pinch scale and translation through existing camera constraints. */
    pinch(factor, anchor, delta) {
        const target = this.read();
        if (!target || !this.navigable())
            return false;
        const zoomed = zoomCanvasCamera(target.camera, factor, anchor, target.geometry);
        this.#transition.set(panCanvasCamera(zoomed, delta, target.geometry));
        return true;
    }
    /** Toggle Fit/native from the actually displayed camera, anchoring only the native destination. */
    doubleClick(anchor) {
        const target = this.read();
        if (!target)
            return false;
        return this.#move((current) => doubleClickCanvasCamera(current.camera, anchor, current.geometry), target.camera.mode === 'fit' ? anchor : null);
    }
    /** Stop an interrupted gesture/animation without moving to its obsolete destination. */
    interrupt() {
        this.#transition.cancel();
    }
    /** Release frame ownership while preserving the enrolled view's lightweight inspection. */
    suspend() {
        this.#transition.cancel();
        this.#target = null;
        this.options.present(null);
    }
    /** Dispose this attachment without retiring another surface's view inspection. */
    dispose() {
        if (this.#disposed)
            return;
        this.suspend();
        this.#disposed = true;
        this.#transition.dispose();
    }
    /** Apply an inspection intent independently of content navigation. */
    #move(destination, anchor, wheel = false) {
        const target = this.read();
        if (!target || !this.navigable())
            return false;
        this.#transition.move(destination(target), target.geometry, anchor, this.options.reducedMotion(), wheel);
        return true;
    }
}
/** Choose the viewport center for toolbar/keyboard zooms without a pointer anchor. */
function center(geometry) {
    return { x: geometry.viewport.width / 2, y: geometry.viewport.height / 2 };
}
