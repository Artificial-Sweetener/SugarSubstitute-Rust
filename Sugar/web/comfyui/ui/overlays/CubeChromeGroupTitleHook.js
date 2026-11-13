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
 * Own the LiteGraph group draw-hook lifecycle for Cube title projection.
 */
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
/** Install and release the managed-group title drawing hook. */
export class CubeChromeGroupTitleHook {
    adapter;
    painter;
    constructor(adapter, painter) {
        this.adapter = adapter;
        this.painter = painter;
    }
    installGroupTitleIconRenderer() {
        const liteGraph = this.adapter?.getLiteGraph?.() ||
            (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
        const GroupRef = liteGraph?.LGraphGroup;
        if (!GroupRef?.prototype) {
            return;
        }
        const prototype = GroupRef.prototype;
        if (typeof prototype.draw !== 'function') {
            return;
        }
        prototype.__sugarcubes_title_icon_renderer = this;
        if (prototype.__sugarcubes_title_icon_patched) {
            return;
        }
        const originalDraw = prototype.draw;
        prototype.__sugarcubes_title_icon_original_draw = originalDraw;
        prototype.__sugarcubes_title_icon_patched = true;
        prototype.draw = function drawSugarCubesGroupTitleIcon(...args) {
            const renderer = prototype.__sugarcubes_title_icon_renderer;
            if (renderer?.shouldDrawGroupTitleIcon?.(this)) {
                return renderer.drawManagedGroupWithTitleIcon(this, originalDraw, args);
            }
            return originalDraw.apply(this, args);
        };
    }
    releaseGroupTitleIconRenderer() {
        const liteGraph = this.adapter?.getLiteGraph?.() ||
            (typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null);
        const GroupRef = liteGraph?.LGraphGroup;
        const prototype = GroupRef?.prototype;
        if (prototype?.__sugarcubes_title_icon_renderer === this) {
            delete prototype.__sugarcubes_title_icon_renderer;
        }
    }
    shouldDrawGroupTitleIcon(group) {
        const metadata = getGroupSugarcubes(group);
        return Boolean(metadata?.managed && metadata.instance_id);
    }
    drawManagedGroupWithTitleIcon(group, originalDraw, args) {
        const titleDescriptor = Object.getOwnPropertyDescriptor(group, 'title');
        const hadTitleDescriptor = Boolean(titleDescriptor);
        try {
            Object.defineProperty(group, 'title', {
                value: '',
                writable: true,
                enumerable: titleDescriptor?.enumerable ?? true,
                configurable: true,
            });
            originalDraw.apply(group, args);
        }
        finally {
            if (hadTitleDescriptor && titleDescriptor) {
                Object.defineProperty(group, 'title', titleDescriptor);
            }
            else {
                delete group.title;
            }
        }
        const ctx = args[1];
        const canvasInstance = args[0];
        if (!ctx ||
            typeof ctx !== 'object' ||
            typeof ctx.save !== 'function' ||
            !canvasInstance ||
            typeof canvasInstance !== 'object') {
            return undefined;
        }
        this.painter.drawGroupTitleIcon(ctx, group, canvasInstance);
        return undefined;
    }
}
