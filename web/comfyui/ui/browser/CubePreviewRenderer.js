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
 * Own the SugarCubes cube browser layer in `web/comfyui/ui/browser/CubePreviewRenderer.js`.
 */
/**
 * Coordinate cube preview renderer behavior for the SugarCubes UI.
 */
export class CubePreviewRenderer {
    adapter;
    helpers;
    placement;
    canvas;
    container;
    status;
    payload;
    name;
    requestKey;
    loading;
    error;
    requestId;
    resizeObserver;
    dragging;
    dragPointerId;
    dragOrigin;
    dragRequestId;
    context;
    pointerHandlersBound;
    pointerHandlers;
    constructor({ adapter = null, helpers = {}, placement = {} } = {}) {
        this.adapter = adapter;
        this.helpers = helpers;
        this.placement = placement;
        this.canvas = null;
        this.container = null;
        this.status = null;
        this.payload = null;
        this.name = null;
        this.requestKey = null;
        this.loading = false;
        this.error = null;
        this.requestId = 0;
        this.resizeObserver = null;
        this.dragging = false;
        this.dragPointerId = null;
        this.dragOrigin = null;
        this.dragRequestId = 0;
        this.context = {
            selected: null,
            selectedId: null,
            busy: false,
        };
        this.pointerHandlersBound = false;
        this.pointerHandlers = null;
    }
    setHelpers(helpers = {}) {
        this.helpers = helpers;
    }
    setPlacementActions(actions = {}) {
        this.placement = actions;
    }
    setContext(context = {}) {
        this.context = { ...this.context, ...context };
    }
    /** Describe whether a preview request is already pending or renderable. */
    getRequestState(requestKey) {
        if (this.requestKey !== requestKey) {
            return 'missing';
        }
        if (this.loading) {
            return 'loading';
        }
        return this.payload && !this.error ? 'ready' : 'missing';
    }
    attach({ canvas, container, status, }) {
        this.canvas = canvas || null;
        this.container = container || null;
        this.status = status || null;
        this.ensureResizeObserver();
        this.bindPointerHandlers();
    }
    dispose() {
        this.detachPointerHandlers();
        this.disconnectResizeObserver();
        this.canvas = null;
        this.container = null;
        this.status = null;
        this.payload = null;
        this.name = null;
        this.requestKey = null;
    }
    update({ payload = null, name = null, requestKey = null, loading = false, error = null, } = {}) {
        this.payload = payload || null;
        this.name = name || null;
        this.requestKey = requestKey || null;
        this.loading = Boolean(loading);
        this.error = error || null;
        this.render();
    }
    setStatus(message) {
        if (!this.status) {
            return;
        }
        this.status.textContent = message || '';
        this.status.style.display = message ? 'block' : 'none';
    }
    ensureResizeObserver() {
        if (this.resizeObserver || typeof ResizeObserver === 'undefined') {
            return;
        }
        if (!this.container) {
            return;
        }
        this.resizeObserver = new ResizeObserver(() => {
            this.render();
        });
        this.resizeObserver.observe(this.container);
    }
    disconnectResizeObserver() {
        if (!this.resizeObserver) {
            return;
        }
        this.resizeObserver.disconnect();
        this.resizeObserver = null;
    }
    computePreviewBounds(payload, ctx) {
        const { computePayloadBounds, readVector2 } = this.helpers;
        if (!computePayloadBounds || !readVector2) {
            return null;
        }
        const nodes = Array.isArray(payload?.nodes) ? payload.nodes : [];
        const markers = Array.isArray(payload?.markers) ? payload.markers : [];
        const entries = nodes.concat(markers);
        let bounds = computePayloadBounds(entries, ctx);
        const baseOrigin = readVector2(payload?.layout?.origin, 0, 0);
        const groups = Array.isArray(payload?.layout?.groups) ? payload.layout.groups : [];
        const updateBounds = (rect) => {
            if (!rect) {
                return;
            }
            const next = {
                minX: rect.x,
                minY: rect.y,
                maxX: rect.x + rect.w,
                maxY: rect.y + rect.h,
            };
            if (!bounds) {
                bounds = next;
                return;
            }
            bounds.minX = Math.min(bounds.minX, next.minX);
            bounds.minY = Math.min(bounds.minY, next.minY);
            bounds.maxX = Math.max(bounds.maxX, next.maxX);
            bounds.maxY = Math.max(bounds.maxY, next.maxY);
        };
        for (const group of groups) {
            if (!group || typeof group !== 'object') {
                continue;
            }
            const bounding = Array.isArray(group.bounding) ? group.bounding : null;
            if (!bounding || bounding.length !== 4) {
                continue;
            }
            const [bx = 0, by = 0, bw = 0, bh = 0] = bounding.map((value) => Number(value) || 0);
            updateBounds({
                x: baseOrigin[0] + bx,
                y: baseOrigin[1] + by,
                w: bw,
                h: bh,
            });
        }
        return bounds;
    }
    drawCubePreview(ctx, payload, bounds, size, defaultAlias) {
        const { drawGhostRect, getPlacementGroupLabel, readVector2, coerceVec2, resolvePreviewRect } = this.helpers;
        if (!payload || !bounds || !size || !drawGhostRect || !readVector2 || !resolvePreviewRect) {
            return;
        }
        const width = size.width;
        const height = size.height;
        const boundsWidth = bounds.maxX - bounds.minX;
        const boundsHeight = bounds.maxY - bounds.minY;
        if (boundsWidth <= 0 || boundsHeight <= 0) {
            return;
        }
        const padding = 12;
        const contentWidth = Math.max(1, width - padding * 2);
        const contentHeight = Math.max(1, height - padding * 2);
        const scale = Math.min(contentWidth / boundsWidth, contentHeight / boundsHeight);
        if (!Number.isFinite(scale) || scale <= 0) {
            return;
        }
        ctx.save();
        ctx.translate(padding - bounds.minX * scale, padding - bounds.minY * scale);
        ctx.scale(scale, scale);
        const baseOrigin = readVector2(payload?.layout?.origin, 0, 0);
        const nodes = Array.isArray(payload?.nodes) ? payload.nodes : [];
        const markers = Array.isArray(payload?.markers) ? payload.markers : [];
        const groups = Array.isArray(payload?.layout?.groups) ? payload.layout.groups : [];
        if (groups.length) {
            for (const group of groups) {
                if (!group || typeof group !== 'object') {
                    continue;
                }
                const bounding = Array.isArray(group.bounding) ? group.bounding : null;
                if (!bounding || bounding.length !== 4) {
                    continue;
                }
                const [bx = 0, by = 0, bw = 0, bh = 0] = bounding.map((value) => Number(value) || 0);
                drawGhostRect(ctx, {
                    x: baseOrigin[0] + bx,
                    y: baseOrigin[1] + by,
                    w: bw,
                    h: bh,
                }, { fill: 'rgba(70, 120, 150, 0.12)', stroke: 'rgba(70, 120, 150, 0.55)', alpha: 0.7 }, scale, getPlacementGroupLabel ? getPlacementGroupLabel(defaultAlias, group) : null);
            }
        }
        else {
            drawGhostRect(ctx, {
                x: bounds.minX,
                y: bounds.minY,
                w: boundsWidth,
                h: boundsHeight,
            }, { fill: 'rgba(70, 120, 150, 0.12)', stroke: 'rgba(70, 120, 150, 0.55)', alpha: 0.5 }, scale, defaultAlias || null);
        }
        for (const entry of nodes) {
            const layout = entry?.layout;
            const pos = coerceVec2 ? coerceVec2(layout?.pos) : null;
            const sizeVec = coerceVec2 ? coerceVec2(layout?.size) : null;
            if (!pos || !sizeVec) {
                continue;
            }
            const rect = resolvePreviewRect(entry, pos, sizeVec, ctx);
            drawGhostRect(ctx, {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
            }, { fill: 'rgba(120, 180, 210, 0.16)', stroke: 'rgba(120, 180, 210, 0.6)', alpha: 0.9 }, scale);
        }
        for (const entry of markers) {
            const layout = entry?.layout;
            const pos = coerceVec2 ? coerceVec2(layout?.pos) : null;
            const sizeVec = coerceVec2 ? coerceVec2(layout?.size) : null;
            if (!pos || !sizeVec) {
                continue;
            }
            const rect = resolvePreviewRect(entry, pos, sizeVec, ctx);
            const kind = typeof entry?.kind === 'string' ? entry.kind : '';
            let style = {
                fill: 'rgba(120, 180, 210, 0.16)',
                stroke: 'rgba(120, 180, 210, 0.6)',
                alpha: 0.9,
            };
            if (kind === 'input') {
                style = {
                    fill: 'rgba(90, 200, 120, 0.2)',
                    stroke: 'rgba(90, 200, 120, 0.7)',
                    alpha: 0.95,
                };
            }
            else if (kind === 'output') {
                style = {
                    fill: 'rgba(90, 140, 240, 0.2)',
                    stroke: 'rgba(90, 140, 240, 0.7)',
                    alpha: 0.95,
                };
            }
            drawGhostRect(ctx, {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
            }, style, scale);
        }
        ctx.restore();
    }
    render() {
        const canvas = this.canvas;
        const container = this.container;
        if (!canvas || !container) {
            return;
        }
        const payload = this.payload;
        if (!payload) {
            canvas.style.display = 'none';
            if (this.loading) {
                this.setStatus('Loading preview...');
            }
            else if (this.error) {
                this.setStatus(this.error);
            }
            else {
                this.setStatus('Select a cube to see the layout preview.');
            }
            return;
        }
        const ctx = canvas.getContext('2d');
        if (!ctx) {
            return;
        }
        const bounds = this.computePreviewBounds(payload, ctx);
        if (!bounds) {
            canvas.style.display = 'none';
            this.setStatus('No layout preview available.');
            return;
        }
        const containerWidth = Math.max(0, container.clientWidth - 16);
        if (!containerWidth) {
            return;
        }
        const boundsWidth = bounds.maxX - bounds.minX || 1;
        const boundsHeight = bounds.maxY - bounds.minY || 1;
        const aspect = boundsHeight / boundsWidth;
        const height = Math.max(120, Math.round(containerWidth * aspect));
        const dpr = this.adapter?.getWindow?.()?.devicePixelRatio || 1;
        canvas.style.display = 'block';
        canvas.style.height = `${height}px`;
        canvas.width = Math.round(containerWidth * dpr);
        canvas.height = Math.round(height * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, containerWidth, height);
        this.drawCubePreview(ctx, payload, bounds, { width: containerWidth, height }, this.name);
        this.setStatus('');
    }
    bindPointerHandlers() {
        if (!this.canvas || this.pointerHandlersBound) {
            return;
        }
        this.pointerHandlersBound = true;
        this.pointerHandlers = {
            down: (event) => this.handlePointerDown(event),
            move: (event) => this.handlePointerMove(event),
            up: (event) => this.handlePointerUp(event),
            cancel: (event) => this.handlePointerCancel(event),
        };
        this.canvas.addEventListener('pointerdown', this.pointerHandlers.down);
        this.canvas.addEventListener('pointermove', this.pointerHandlers.move);
        this.canvas.addEventListener('pointerup', this.pointerHandlers.up);
        this.canvas.addEventListener('pointercancel', this.pointerHandlers.cancel);
    }
    detachPointerHandlers() {
        if (!this.canvas || !this.pointerHandlersBound || !this.pointerHandlers) {
            return;
        }
        this.pointerHandlersBound = false;
        this.canvas.removeEventListener('pointerdown', this.pointerHandlers.down);
        this.canvas.removeEventListener('pointermove', this.pointerHandlers.move);
        this.canvas.removeEventListener('pointerup', this.pointerHandlers.up);
        this.canvas.removeEventListener('pointercancel', this.pointerHandlers.cancel);
        this.pointerHandlers = null;
    }
    updateDragPlacementOrigin(event) {
        const computeOrigin = this.placement?.computeOriginFromEvent;
        const origin = computeOrigin ? computeOrigin(event) : null;
        if (!origin) {
            return;
        }
        this.dragOrigin = origin;
        const placementState = this.placement?.getState?.();
        if (placementState?.active) {
            this.placement?.setOrigin?.(origin);
        }
        this.placement?.setDirty?.();
    }
    handlePointerDown(event) {
        if (event.button !== 0) {
            return;
        }
        const cubeId = this.context.selectedId;
        const defaultAlias = this.context.selected;
        if (!cubeId || this.context.busy) {
            return;
        }
        event.preventDefault();
        event.stopPropagation();
        this.dragging = true;
        this.dragPointerId = event.pointerId ?? null;
        this.dragOrigin = null;
        this.dragRequestId += 1;
        const requestId = this.dragRequestId;
        if (this.canvas?.setPointerCapture && event.pointerId != null) {
            try {
                this.canvas.setPointerCapture(event.pointerId);
            }
            catch (_error) {
                // ignore pointer capture failures
            }
        }
        this.canvas?.classList.add('is-dragging');
        this.updateDragPlacementOrigin(event);
        void Promise.resolve(this.placement?.start?.(cubeId, { closeBrowser: false, defaultAlias })).then(() => {
            if (this.dragRequestId !== requestId) {
                return;
            }
            const placementState = this.placement?.getState?.();
            if (!this.dragging && placementState?.active && placementState?.cubeId === cubeId) {
                this.placement?.stop?.('Placement cancelled.');
                return;
            }
            if (this.dragOrigin && placementState?.active) {
                this.placement?.setOrigin?.(this.dragOrigin);
                this.placement?.setDirty?.();
            }
        });
    }
    handlePointerMove(event) {
        if (!this.dragging) {
            return;
        }
        event.preventDefault();
        this.updateDragPlacementOrigin(event);
    }
    endPreviewDrag(event, placeOnRelease) {
        if (!this.dragging) {
            return;
        }
        if (this.canvas?.releasePointerCapture && this.dragPointerId != null) {
            try {
                this.canvas.releasePointerCapture(this.dragPointerId);
            }
            catch (_error) {
                // ignore pointer release failures
            }
        }
        this.canvas?.classList.remove('is-dragging');
        this.dragging = false;
        this.dragPointerId = null;
        this.dragOrigin = null;
        const placementState = this.placement?.getState?.();
        if (!placementState?.active || placementState.commitInProgress) {
            return;
        }
        if (placeOnRelease && this.placement?.isPointerOverCanvas?.(event)) {
            this.placement?.setCommitInProgress?.(true);
            void Promise.resolve(this.placement?.commit?.()).finally(() => {
                this.placement?.setCommitInProgress?.(false);
            });
            return;
        }
        this.placement?.stop?.('Placement cancelled.');
    }
    handlePointerUp(event) {
        this.endPreviewDrag(event, true);
    }
    handlePointerCancel(event) {
        this.endPreviewDrag(event, false);
    }
}
