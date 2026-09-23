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
 * Own Cube chrome hit testing, hover state, and action dispatch.
 */
/** Route pointer input through published Cube chrome regions. */
export class CubeChromeInteraction {
    options;
    hoveredKey = null;
    hoveredInstance = null;
    hoveredBadgeInstance = null;
    hoveredBadgeKey = null;
    constructor(options) {
        this.options = options;
    }
    buildMenuOptions({ metadata, }) {
        if (metadata.kind === 'draft') {
            return [
                {
                    title: 'Save SugarCube',
                    callback: () => this.options.getActions().onSaveDraft?.(metadata),
                },
            ];
        }
        const actions = this.options.getActions();
        const classification = actions.getLibraryClassification?.(metadata) ?? null;
        if (classification?.access === 'read_only') {
            const entries = [];
            if (classification.permittedOperations.has('keep')) {
                entries.push({
                    title: 'Keep workflow copy',
                    callback: () => actions.onKeepWorkflowCube?.(metadata),
                });
            }
            if (classification.permittedOperations.has('capture')) {
                entries.push({
                    title: 'Capture Cube',
                    callback: () => actions.onCaptureWorkflowCube?.(metadata),
                });
            }
            if (classification.permittedOperations.has('track_source')) {
                entries.push({
                    title: 'Synchronize home source…',
                    callback: () => actions.onSyncWorkflowCubeSource?.(metadata),
                });
            }
            if (classification.permittedOperations.has('fork')) {
                entries.push({
                    title: 'Fork to Local Cubes…',
                    callback: () => actions.onForkWorkflowCube?.(metadata),
                });
            }
            return entries;
        }
        return [
            {
                title: 'Save cube implementation',
                callback: () => this.options.getActions().onSaveImplementation?.(metadata),
            },
        ];
    }
    clear() {
        this.hoveredKey = null;
        this.hoveredInstance = null;
        this.hoveredBadgeInstance = null;
        this.hoveredBadgeKey = null;
    }
    getHoverState() {
        return {
            hoveredKey: this.hoveredKey,
            hoveredInstance: this.hoveredInstance,
            hoveredBadgeInstance: this.hoveredBadgeInstance,
            hoveredBadgeKey: this.hoveredBadgeKey,
        };
    }
    handleMouseDown(event, canvasInstance) {
        if (!event || !canvasInstance || !this.options.getHitRegions().length) {
            return false;
        }
        const point = this.convertEventToCanvasPoint(event, canvasInstance);
        if (!point) {
            return false;
        }
        for (const region of this.options.getHitRegions()) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                const handlers = {
                    'swap-left': () => this.options.getActions().onSwapLeft?.(region.metadata),
                    'swap-right': () => this.options.getActions().onSwapRight?.(region.metadata),
                    menu: () => {
                        const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
                        if (!liteGraph?.ContextMenu) {
                            return;
                        }
                        const options = this.buildMenuOptions({
                            metadata: region.metadata,
                            isDirty: Boolean(region.metadata?.dirty),
                            flavors: region.flavorOptions || [],
                        });
                        new liteGraph.ContextMenu(options, { event });
                    },
                };
                const handler = handlers[region.key];
                if (handler) {
                    handler();
                    return true;
                }
                return false;
            }
        }
        return false;
    }
    handlePointerMove(event, canvasInstance) {
        if (!event || !canvasInstance) {
            return false;
        }
        const point = this.convertEventToCanvasPoint(event, canvasInstance);
        if (!point) {
            return false;
        }
        let nextKey = null;
        let nextInstance = null;
        let nextBadgeInstance = null;
        let nextBadgeKey = null;
        for (const region of this.options.getBadgeRegions()) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                nextBadgeInstance = region.instanceId || null;
                nextBadgeKey = region.key || null;
                break;
            }
        }
        for (const region of this.options.getHitRegions()) {
            const { x, y, w, h } = region.rect;
            if (point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h) {
                nextKey = region.key;
                nextInstance = region.instanceId || null;
                break;
            }
        }
        if (nextKey !== this.hoveredKey ||
            nextInstance !== this.hoveredInstance ||
            nextBadgeInstance !== this.hoveredBadgeInstance ||
            nextBadgeKey !== this.hoveredBadgeKey) {
            this.hoveredKey = nextKey;
            this.hoveredInstance = nextInstance;
            this.hoveredBadgeInstance = nextBadgeInstance;
            this.hoveredBadgeKey = nextBadgeKey;
            this.options.requestRedraw();
        }
        return Boolean(nextKey);
    }
    convertEventToCanvasPoint(event, canvasInstance) {
        const canvasElement = canvasInstance?.canvas ?? null;
        if (!canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
            return null;
        }
        if (typeof canvasInstance.convertEventToCanvasOffset === 'function') {
            try {
                const converted = canvasInstance.convertEventToCanvasOffset(event);
                if (Array.isArray(converted) && converted.length >= 2) {
                    const x = Number(converted[0]);
                    const y = Number(converted[1]);
                    if (Number.isFinite(x) && Number.isFinite(y)) {
                        return [x, y];
                    }
                }
            }
            catch (_error) {
                // ignore event conversion failures
            }
        }
        const rect = canvasElement.getBoundingClientRect();
        const relative = [event.clientX - rect.left, event.clientY - rect.top];
        return this.convertCanvasPoint(canvasInstance, relative);
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
        catch (_error) {
            // ignore conversion failures
        }
        return null;
    }
}
