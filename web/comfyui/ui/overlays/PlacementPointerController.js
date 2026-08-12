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
 * Own placement coordinate conversion and pointer commit state.
 */
import { isRecord } from '../types/common.js';
/** Resolve canvas-space placement state from host pointer input. */
export class PlacementPointerController {
    options;
    adapter;
    state;
    logger;
    constructor(options) {
        this.options = options;
        this.adapter = options.adapter;
        this.state = options.state;
        this.logger = options.logger;
    }
    getCanvas() {
        const canvas = this.adapter?.getApp?.()?.canvas ?? this.adapter?.getApp?.()?.graph?.canvas;
        return isRecord(canvas) ? canvas : null;
    }
    getState() {
        return this.state;
    }
    setOrigin(origin) {
        this.state.origin = origin;
    }
    setCommitInProgress(value) {
        this.state.commitInProgress = Boolean(value);
    }
    setDirty() {
        this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
    }
    computeOriginFromEvent(event) {
        const canvasInstance = this.getCanvas();
        const canvasElement = canvasInstance?.canvas ?? null;
        if (!canvasInstance || !canvasElement) {
            return null;
        }
        if (typeof canvasElement.getBoundingClientRect !== 'function') {
            return null;
        }
        const rect = canvasElement.getBoundingClientRect();
        const relative = [event.clientX - rect.left, event.clientY - rect.top];
        return this.convertCanvasPoint(canvasInstance, relative);
    }
    isPointerOverCanvas(event) {
        const canvasInstance = this.getCanvas();
        const canvasElement = canvasInstance?.canvas ?? null;
        if (!canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
            return false;
        }
        const rect = canvasElement.getBoundingClientRect();
        return (event.clientX >= rect.left &&
            event.clientX <= rect.right &&
            event.clientY >= rect.top &&
            event.clientY <= rect.bottom);
    }
    convertCanvasPoint(canvasInstance, point) {
        if (!canvasInstance || !Array.isArray(point)) {
            return null;
        }
        try {
            if (typeof canvasInstance.convertCanvasToOffset === 'function') {
                const converted = canvasInstance.convertCanvasToOffset(point);
                if (Array.isArray(converted) && converted.length >= 2) {
                    const x = Number(converted[0]);
                    const y = Number(converted[1]);
                    if (Number.isFinite(x) && Number.isFinite(y)) {
                        return [x, y];
                    }
                }
            }
            const ds = canvasInstance.ds;
            const scale = Number(ds?.scale) || 1;
            const offset = Array.isArray(ds?.offset) ? ds.offset : [0, 0];
            const x = point[0] / scale - (offset[0] ?? 0);
            const y = point[1] / scale - (offset[1] ?? 0);
            if (Number.isFinite(x) && Number.isFinite(y)) {
                return [x, y];
            }
        }
        catch (error) {
            this.logger?.warn?.('SugarCubes -> convertCanvasPoint failed', error);
        }
        return null;
    }
    computeDropOrigin() {
        const canvasInstance = this.getCanvas();
        if (!canvasInstance) {
            return [0, 0];
        }
        const lastMouse = canvasInstance.last_mouse_position;
        if (Array.isArray(lastMouse) &&
            Number.isFinite(lastMouse[0]) &&
            Number.isFinite(lastMouse[1])) {
            const converted = this.convertCanvasPoint(canvasInstance, [
                Number(lastMouse[0]),
                Number(lastMouse[1]),
            ]);
            if (converted) {
                return converted;
            }
        }
        try {
            const canvasElement = canvasInstance.canvas ?? null;
            if (canvasElement && typeof canvasElement.getBoundingClientRect === 'function') {
                const rect = canvasElement.getBoundingClientRect();
                const relative = [rect.width / 2, rect.height / 2];
                const converted = this.convertCanvasPoint(canvasInstance, relative);
                if (converted) {
                    return converted;
                }
            }
        }
        catch (_error) {
            // ignore viewport conversion issues
        }
        const ds = canvasInstance.ds ?? null;
        if (ds) {
            const offset = Array.isArray(ds.offset) ? ds.offset : [0, 0];
            const x = -Number(offset[0] ?? 0);
            const y = -Number(offset[1] ?? 0);
            if (Number.isFinite(x) && Number.isFinite(y)) {
                return [x, y];
            }
        }
        return [0, 0];
    }
    resolvePlacementOrigin(canvasInstance, event) {
        if (event && typeof event.clientX === 'number' && typeof event.clientY === 'number') {
            const fromEvent = this.computeOriginFromEvent(event);
            if (fromEvent) {
                return fromEvent;
            }
        }
        const lastMouse = canvasInstance?.last_mouse_position;
        if (Array.isArray(lastMouse) &&
            Number.isFinite(lastMouse[0]) &&
            Number.isFinite(lastMouse[1])) {
            const converted = this.convertCanvasPoint(canvasInstance, [
                Number(lastMouse[0]),
                Number(lastMouse[1]),
            ]);
            if (converted) {
                return converted;
            }
        }
        return Array.isArray(this.state.origin) ? this.state.origin : this.computeDropOrigin();
    }
    handlePlacementMouseDown(event, canvasInstance, overlayElement = null) {
        if (!this.state.active || this.state.commitInProgress) {
            return false;
        }
        const button = typeof event?.button === 'number' ? event.button : 0;
        if (button === 0) {
            event?.preventDefault?.();
            event?.stopPropagation?.();
            event?.stopImmediatePropagation?.();
            if (event) {
                event.cancelBubble = true;
            }
            const pointerId = 'pointerId' in event ? event.pointerId : null;
            if (overlayElement?.setPointerCapture && pointerId != null) {
                try {
                    overlayElement.setPointerCapture(pointerId);
                }
                catch (_error) {
                    // ignore pointer capture failures
                }
            }
            this.state.origin = this.resolvePlacementOrigin(canvasInstance, event);
            this.state.commitInProgress = true;
            void this.options.commit().finally(() => {
                this.state.commitInProgress = false;
            });
            return true;
        }
        if (button === 2) {
            event?.preventDefault?.();
            event?.stopPropagation?.();
            event?.stopImmediatePropagation?.();
            if (event) {
                event.cancelBubble = true;
            }
            this.options.stop('Placement cancelled.');
            return true;
        }
        return false;
    }
}
