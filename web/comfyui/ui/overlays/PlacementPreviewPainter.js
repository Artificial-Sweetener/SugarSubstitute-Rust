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
 * Paint immutable Cube placement previews on the active canvas.
 */
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import { readVector2 } from '../graph/VectorUtils.js';
import { computePayloadBounds, drawGhostRect, getPlacementGroupLabel, resolvePreviewRect, } from './PlacementHelpers.js';
const PLACEMENT_GROUP_FILL = 'rgba(70, 120, 150, 0.12)';
const PLACEMENT_GROUP_STROKE = 'rgba(70, 120, 150, 0.55)';
const PLACEMENT_NODE_FILL = 'rgba(120, 180, 210, 0.16)';
const PLACEMENT_NODE_STROKE = 'rgba(120, 180, 210, 0.6)';
const PLACEMENT_INPUT_FILL = 'rgba(90, 200, 120, 0.2)';
const PLACEMENT_INPUT_STROKE = 'rgba(90, 200, 120, 0.7)';
const PLACEMENT_OUTPUT_FILL = 'rgba(90, 140, 240, 0.2)';
const PLACEMENT_OUTPUT_STROKE = 'rgba(90, 140, 240, 0.7)';
/** Render one placement state without owning its lifecycle. */
export class PlacementPreviewPainter {
    adapter;
    state;
    constructor(adapter, state) {
        this.adapter = adapter;
        this.state = state;
    }
    render(ctx, canvasInstance) {
        if (!this.state.active || !this.state.payload) {
            return;
        }
        if (!canvasInstance || canvasInstance.graph !== this.adapter?.getApp?.()?.graph) {
            return;
        }
        const payload = this.state.payload;
        const baseOrigin = readVector2(payload?.layout?.origin, 0, 0);
        const currentOrigin = Array.isArray(this.state.origin) ? this.state.origin : baseOrigin;
        const shiftX = currentOrigin[0] - baseOrigin[0];
        const shiftY = currentOrigin[1] - baseOrigin[1];
        const scale = Number(canvasInstance?.ds?.scale) || 1;
        const liteGraph = this.adapter?.getLiteGraph?.() || null;
        const nodes = Array.isArray(payload?.nodes) ? payload.nodes : [];
        const markers = Array.isArray(payload?.markers) ? payload.markers : [];
        const entries = nodes.concat(markers);
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
                const rect = {
                    x: baseOrigin[0] + bx + shiftX,
                    y: baseOrigin[1] + by + shiftY,
                    w: bw,
                    h: bh,
                };
                drawGhostRect(ctx, rect, { fill: PLACEMENT_GROUP_FILL, stroke: PLACEMENT_GROUP_STROKE, alpha: 0.8 }, scale, getPlacementGroupLabel(this.state.defaultAlias, group, getGroupSugarcubes));
            }
        }
        else {
            const bounds = computePayloadBounds(entries, ctx, liteGraph);
            if (bounds) {
                drawGhostRect(ctx, {
                    x: bounds.minX + shiftX,
                    y: bounds.minY + shiftY,
                    w: bounds.maxX - bounds.minX,
                    h: bounds.maxY - bounds.minY,
                }, { fill: PLACEMENT_GROUP_FILL, stroke: PLACEMENT_GROUP_STROKE, alpha: 0.6 }, scale, this.state.defaultAlias);
            }
        }
        for (const entry of nodes) {
            const layout = entry?.layout;
            if (!Array.isArray(layout?.pos) || !Array.isArray(layout?.size)) {
                continue;
            }
            const pos = readVector2(layout.pos, 0, 0);
            const size = readVector2(layout.size, 0, 0);
            const rect = resolvePreviewRect(entry, pos, size, ctx, liteGraph);
            drawGhostRect(ctx, {
                x: rect.x + shiftX,
                y: rect.y + shiftY,
                w: rect.w,
                h: rect.h,
            }, { fill: PLACEMENT_NODE_FILL, stroke: PLACEMENT_NODE_STROKE, alpha: 0.9 }, scale);
        }
        for (const entry of markers) {
            const layout = entry?.layout;
            if (!Array.isArray(layout?.pos) || !Array.isArray(layout?.size)) {
                continue;
            }
            const pos = readVector2(layout.pos, 0, 0);
            const size = readVector2(layout.size, 0, 0);
            const rect = resolvePreviewRect(entry, pos, size, ctx, liteGraph);
            const kind = typeof entry?.kind === 'string' ? entry.kind : '';
            let style = { fill: PLACEMENT_NODE_FILL, stroke: PLACEMENT_NODE_STROKE, alpha: 0.9 };
            if (kind === 'input') {
                style = { fill: PLACEMENT_INPUT_FILL, stroke: PLACEMENT_INPUT_STROKE, alpha: 0.95 };
            }
            else if (kind === 'output') {
                style = { fill: PLACEMENT_OUTPUT_FILL, stroke: PLACEMENT_OUTPUT_STROKE, alpha: 0.95 };
            }
            drawGhostRect(ctx, {
                x: rect.x + shiftX,
                y: rect.y + shiftY,
                w: rect.w + 0,
                h: rect.h + 0,
            }, style, scale);
        }
    }
}
