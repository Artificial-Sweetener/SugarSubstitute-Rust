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
 * Own read-only graph navigation for the create-cube modal backdrop.
 */
import { isRecord } from '../types/common.js';
function isFiniteNumber(value) {
    return Number.isFinite(Number(value));
}
/**
 * Translate create-modal backdrop input into viewport-only graph navigation.
 */
export class CreateModalGraphNavigator {
    adapter;
    overlay;
    dialog;
    isPanning;
    pointerId;
    lastClientX;
    lastClientY;
    originalOverlayCursor;
    originalDialogCursor;
    listeners;
    windowListeners;
    windowListenersAttached;
    constructor({ adapter = null } = {}) {
        this.adapter = adapter;
        this.overlay = null;
        this.dialog = null;
        this.isPanning = false;
        this.pointerId = null;
        this.lastClientX = 0;
        this.lastClientY = 0;
        this.originalOverlayCursor = '';
        this.originalDialogCursor = '';
        this.listeners = [
            ['pointerdown', (event) => this.handlePointerDown(event)],
            ['pointermove', (event) => this.handlePointerMove(event)],
            ['pointerup', (event) => this.handlePointerEnd(event)],
            ['pointercancel', (event) => this.handlePointerEnd(event)],
            ['wheel', (event) => this.handleWheel(event)],
            ['click', (event) => this.blockBackdropEvent(event)],
            ['dblclick', (event) => this.blockBackdropEvent(event)],
            ['contextmenu', (event) => this.blockBackdropEvent(event)],
        ];
        this.windowListeners = [
            ['pointermove', (event) => this.handlePointerMove(event)],
            ['pointerup', (event) => this.handlePointerEnd(event)],
            ['pointercancel', (event) => this.handlePointerEnd(event)],
        ];
        this.windowListenersAttached = false;
    }
    attach(overlay, dialog) {
        this.detach();
        if (!overlay) {
            return;
        }
        this.overlay = overlay;
        this.dialog = dialog || null;
        this.originalOverlayCursor = overlay.style.cursor;
        this.originalDialogCursor = this.dialog?.style?.cursor || '';
        this.setBackdropCursor('grab');
        if (this.dialog?.style) {
            this.dialog.style.cursor = this.originalDialogCursor || 'auto';
        }
        for (const [type, listener] of this.listeners) {
            overlay.addEventListener(type, listener, true);
        }
    }
    detach() {
        this.detachWindowListeners();
        if (this.overlay) {
            for (const [type, listener] of this.listeners) {
                this.overlay.removeEventListener(type, listener, true);
            }
            this.overlay.style.cursor = this.originalOverlayCursor;
        }
        if (this.dialog?.style) {
            this.dialog.style.cursor = this.originalDialogCursor;
        }
        this.isPanning = false;
        this.pointerId = null;
        this.overlay = null;
        this.dialog = null;
        this.originalOverlayCursor = '';
        this.originalDialogCursor = '';
    }
    blockBackdropEvent(event) {
        if (!this.isNavigationEvent(event)) {
            return false;
        }
        event.preventDefault();
        event.stopPropagation();
        return true;
    }
    isNavigationEvent(event) {
        if (this.isCapturedPanEvent(event)) {
            return true;
        }
        return this.isBackdropEvent(event);
    }
    isBackdropEvent(event) {
        const target = event?.target;
        if (!target || !this.overlay) {
            return false;
        }
        if (target instanceof Node && this.dialog?.contains(target)) {
            return false;
        }
        return (target === this.overlay || Boolean(target instanceof Node && this.overlay.contains(target)));
    }
    isCapturedPanEvent(event) {
        if (!this.isPanning) {
            return false;
        }
        const pointerId = event?.pointerId ?? null;
        return pointerId == null || this.pointerId == null || pointerId === this.pointerId;
    }
    handlePointerDown(event) {
        if (!this.blockBackdropEvent(event)) {
            return;
        }
        const isPrimary = event.isPrimary !== false;
        const isPanButton = typeof event.button !== 'number' || event.button === 0 || event.button === 1;
        if (!isPrimary || !isPanButton) {
            return;
        }
        this.isPanning = true;
        this.pointerId = event.pointerId ?? null;
        this.lastClientX = Number(event.clientX) || 0;
        this.lastClientY = Number(event.clientY) || 0;
        this.attachWindowListeners();
        this.setBackdropCursor('grabbing');
        if (this.pointerId != null && typeof this.overlay?.setPointerCapture === 'function') {
            try {
                this.overlay.setPointerCapture(this.pointerId);
            }
            catch (_error) {
                // Pointer capture can fail in tests or if the browser already released it.
            }
        }
    }
    handlePointerMove(event) {
        if (!this.blockBackdropEvent(event) || !this.isPanning) {
            return;
        }
        const clientX = Number(event.clientX) || 0;
        const clientY = Number(event.clientY) || 0;
        const dx = clientX - this.lastClientX;
        const dy = clientY - this.lastClientY;
        this.lastClientX = clientX;
        this.lastClientY = clientY;
        this.panBy(dx, dy);
    }
    handlePointerEnd(event) {
        if (!this.blockBackdropEvent(event)) {
            return;
        }
        const pointerId = event.pointerId ?? null;
        if (pointerId != null &&
            typeof this.overlay?.releasePointerCapture === 'function' &&
            this.pointerId === pointerId) {
            try {
                this.overlay.releasePointerCapture(pointerId);
            }
            catch (_error) {
                // Pointer capture may already be released by the browser.
            }
        }
        this.isPanning = false;
        this.pointerId = null;
        this.setBackdropCursor('grab');
        this.detachWindowListeners();
    }
    handleWheel(event) {
        if (!this.blockBackdropEvent(event)) {
            return;
        }
        const canvas = this.resolveCanvas();
        const ds = canvas?.ds;
        if (!ds || typeof ds.changeScale !== 'function' || !isFiniteNumber(ds.scale)) {
            return;
        }
        const zoomSpeed = Number(canvas?.zoom_speed) || 1.1;
        const nextScale = Number(event.deltaY) < 0 ? Number(ds.scale) * zoomSpeed : Number(ds.scale) / zoomSpeed;
        ds.changeScale(nextScale, [Number(event.clientX) || 0, Number(event.clientY) || 0], false);
        this.requestRedraw(canvas);
    }
    panBy(dx, dy) {
        const canvas = this.resolveCanvas();
        const ds = canvas?.ds;
        if (!ds ||
            !Array.isArray(ds.offset) ||
            ds.offset.length < 2 ||
            !isFiniteNumber(ds.scale) ||
            Number(ds.scale) === 0) {
            return;
        }
        ds.offset[0] = (ds.offset[0] ?? 0) + dx / Number(ds.scale);
        ds.offset[1] = (ds.offset[1] ?? 0) + dy / Number(ds.scale);
        this.requestRedraw(canvas);
    }
    resolveCanvas() {
        const windowRef = this.adapter?.getWindow?.() || (typeof window !== 'undefined' ? window : null);
        const windowApp = isRecord(windowRef?.app) ? windowRef.app : null;
        const app = this.adapter?.getApp?.() || windowApp;
        const windowCanvasHost = isRecord(windowRef?.LGraphCanvas) ? windowRef.LGraphCanvas : {};
        const globalRecord = isRecord(globalThis) ? globalThis : {};
        const globalCanvasHost = isRecord(globalRecord.LGraphCanvas) ? globalRecord.LGraphCanvas : {};
        const candidates = [
            this.adapter?.getCanvas?.(),
            app?.canvas,
            app?.graph?.canvas,
            windowCanvasHost.active_canvas,
            globalCanvasHost.active_canvas,
        ];
        return candidates.find((candidate) => isRecord(candidate)) ?? null;
    }
    requestRedraw(canvas) {
        canvas?.setDirty?.(true, true);
    }
    setBackdropCursor(cursor) {
        if (this.overlay?.style) {
            this.overlay.style.cursor = cursor;
        }
    }
    attachWindowListeners() {
        const windowRef = this.adapter?.getWindow?.() || (typeof window !== 'undefined' ? window : null);
        if (this.windowListenersAttached || !windowRef) {
            return;
        }
        for (const [type, listener] of this.windowListeners) {
            windowRef.addEventListener(type, listener, true);
        }
        this.windowListenersAttached = true;
    }
    detachWindowListeners() {
        const windowRef = this.adapter?.getWindow?.() || (typeof window !== 'undefined' ? window : null);
        if (!this.windowListenersAttached || !windowRef) {
            this.windowListenersAttached = false;
            return;
        }
        for (const [type, listener] of this.windowListeners) {
            windowRef.removeEventListener(type, listener, true);
        }
        this.windowListenersAttached = false;
    }
}
