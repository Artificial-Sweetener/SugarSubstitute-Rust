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
 * Own installation and watchdog lifecycle for canvas overlay draw hooks.
 */
/** Maintain the active host canvas draw hook across renderer replacement. */
export class OverlayDrawHookLifecycle {
    adapter;
    scheduler;
    proximity;
    placement;
    chrome;
    proximityGraphMutations;
    overlayDrawHooked = false;
    overlayWatchdog = { timerId: null, attempts: 0 };
    constructor(adapter, scheduler, proximity, placement, chrome, proximityGraphMutations) {
        this.adapter = adapter;
        this.scheduler = scheduler;
        this.proximity = proximity;
        this.placement = placement;
        this.chrome = chrome;
        this.proximityGraphMutations = proximityGraphMutations;
    }
    dispose() {
        if (this.overlayWatchdog.timerId != null) {
            this.adapter?.getWindow?.()?.clearInterval?.(this.overlayWatchdog.timerId);
            this.overlayWatchdog.timerId = null;
        }
    }
    isOverlayHookActive(canvas) {
        if (!canvas) {
            return false;
        }
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const proto = liteGraph?.LGraphCanvas?.prototype ?? null;
        if (proto?.__sugarcubes_overlay_hooked) {
            return true;
        }
        return Boolean(canvas.onDrawForeground?.__sugarcubes_overlay_hooked ||
            canvas.onDrawBackground?.__sugarcubes_overlay_hooked);
    }
    ensureOverlayHook(attempt = 0) {
        const appRef = this.adapter?.getApp?.() || null;
        const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null);
        const MAX_RETRIES = 60;
        const RETRY_DELAY_MS = 250;
        if (!canvas) {
            if (attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
            }
            return;
        }
        if (this.overlayDrawHooked && this.isOverlayHookActive(canvas)) {
            return;
        }
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const canvasProto = liteGraph?.LGraphCanvas?.prototype ?? null;
        const manager = this;
        if (canvasProto && typeof canvasProto.drawForeground === 'function') {
            if (!canvasProto.__sugarcubes_overlay_hooked) {
                canvasProto.__sugarcubes_overlay_hooked = true;
                const originalDrawForeground = canvasProto.drawForeground;
                canvasProto.drawForeground = function drawForeground(ctx, ...args) {
                    const result = originalDrawForeground.call(this, ctx, ...args);
                    try {
                        manager.placement.render(ctx, this);
                        manager.chrome.render(ctx, this);
                    }
                    catch (error) {
                        manager.adapter
                            ?.getConsole?.()
                            ?.error?.('SugarCubes: drawForeground overlay failed', error);
                    }
                    return result;
                };
            }
            this.overlayDrawHooked = true;
        }
        if (typeof canvas.onDrawForeground === 'function') {
            const previous = canvas.onDrawForeground;
            const wrappedForeground = function onDrawForeground(ctx, ...args) {
                try {
                    previous.call(this, ctx, ...args);
                }
                catch (error) {
                    manager.adapter
                        ?.getConsole?.()
                        ?.error?.('SugarCubes: onDrawForeground wrapper failed', error);
                }
                manager.placement.render(ctx, this);
                manager.chrome.render(ctx, this);
            };
            wrappedForeground.__sugarcubes_overlay_hooked = true;
            canvas.onDrawForeground = wrappedForeground;
            this.overlayDrawHooked = true;
            if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
                this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
            }
        }
        const previous = typeof canvas.onDrawBackground === 'function' ? canvas.onDrawBackground : null;
        const wrappedBackground = function onDrawBackground(ctx, ...args) {
            try {
                previous?.call(this, ctx, ...args);
            }
            catch (error) {
                manager.adapter
                    ?.getConsole?.()
                    ?.error?.('SugarCubes: onDrawBackground wrapper failed', error);
            }
            try {
                manager.proximityGraphMutations.attach(this.graph);
                manager.proximity.ensurePreview(this.graph);
                manager.proximity.render(ctx, this);
                manager.placement.render(ctx, this);
            }
            catch (error) {
                manager.adapter?.getConsole?.()?.error?.('SugarCubes: background overlays failed', error);
            }
        };
        wrappedBackground.__sugarcubes_overlay_hooked = true;
        canvas.onDrawBackground = wrappedBackground;
        this.overlayDrawHooked = true;
        if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
            this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
        }
    }
    startOverlayWatchdog() {
        if (this.overlayWatchdog.timerId != null) {
            return;
        }
        this.overlayWatchdog.attempts = 0;
        const windowRef = this.adapter?.getWindow?.() || null;
        if (!windowRef) {
            return;
        }
        this.overlayWatchdog.timerId = windowRef.setInterval(() => {
            this.overlayWatchdog.attempts += 1;
            this.ensureOverlayHook();
            const appRef = this.adapter?.getApp?.() || null;
            const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null);
            if (canvas && this.isOverlayHookActive(canvas)) {
                windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
                this.overlayWatchdog.timerId = null;
                return;
            }
            if (this.overlayWatchdog.attempts >= 20) {
                windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
                this.overlayWatchdog.timerId = null;
            }
        }, 500);
    }
}
