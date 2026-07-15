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
 * Own the SugarCubes overlay rendering layer in `web/comfyui/ui/overlays/PlacementHelpers.js`.
 */
import { coerceVec2 } from '../graph/VectorUtils.js';
import { resolveInstanceDisplayName } from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
/**
 * Read layout flags.
 */
export function readLayoutFlags(layout) {
    if (!layout) {
        return null;
    }
    const direct = layout.flags;
    if (isRecord(direct)) {
        return direct;
    }
    const extraFlags = isRecord(layout.extra) ? layout.extra.flags : null;
    if (isRecord(extraFlags)) {
        return extraFlags;
    }
    return null;
}
/**
 * Resolve collapsed preview size.
 */
export function resolveCollapsedPreviewSize(layout, size, ctx, liteGraph) {
    const title = typeof layout?.title === 'string' ? layout.title : '';
    const width = Number(size?.[0]) || 140;
    const titleHeight = Number(liteGraph?.NODE_TITLE_HEIGHT) || 30;
    const collapsedWidthDefault = Number(liteGraph?.NODE_COLLAPSED_WIDTH) || 80;
    let collapsedWidth = Math.min(width, collapsedWidthDefault);
    if (ctx && title) {
        const previousFont = ctx.font;
        const fontSize = Number(liteGraph?.NODE_TEXT_SIZE) || 14;
        const fontFamily = typeof liteGraph?.NODE_FONT === 'string' && liteGraph.NODE_FONT
            ? liteGraph.NODE_FONT
            : 'Arial';
        ctx.font = `${fontSize}px ${fontFamily}`;
        const measured = ctx.measureText(title).width + titleHeight * 2;
        if (Number.isFinite(measured) && measured > 0) {
            collapsedWidth = Math.min(width, measured);
        }
        ctx.font = previousFont;
    }
    return [collapsedWidth, titleHeight];
}
/**
 * Resolve preview size.
 */
export function resolvePreviewSize(entry, size, ctx, liteGraph) {
    const layout = entry?.layout;
    const flags = readLayoutFlags(layout);
    if (!flags || flags.collapsed !== true) {
        return size;
    }
    return resolveCollapsedPreviewSize(layout, size, ctx, liteGraph);
}
/**
 * Resolve preview rect.
 */
export function resolvePreviewRect(entry, pos, size, ctx, liteGraph) {
    const layout = entry?.layout;
    const flags = readLayoutFlags(layout);
    const resolvedSize = resolvePreviewSize(entry, size, ctx, liteGraph);
    const titleHeight = Number(liteGraph?.NODE_TITLE_HEIGHT) || 30;
    const y = flags && flags.collapsed === true ? pos[1] - titleHeight : pos[1];
    return {
        x: pos[0],
        y,
        w: resolvedSize[0],
        h: resolvedSize[1],
    };
}
/**
 * Read layout style.
 */
export function readLayoutStyle(layout) {
    if (!layout) {
        return null;
    }
    const direct = layout.style;
    if (isRecord(direct)) {
        return direct;
    }
    const extraStyle = isRecord(layout.extra) ? layout.extra.style : null;
    if (isRecord(extraStyle)) {
        return extraStyle;
    }
    return null;
}
/**
 * Compute payload bounds.
 */
export function computePayloadBounds(entries, ctx = null, liteGraph = null) {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const entry of entries) {
        const layout = entry?.layout;
        if (!layout) {
            continue;
        }
        const pos = coerceVec2(layout.pos);
        const size = coerceVec2(layout.size);
        if (!pos || !size) {
            continue;
        }
        const rect = resolvePreviewRect(entry, pos, size, ctx, liteGraph);
        minX = Math.min(minX, rect.x);
        minY = Math.min(minY, rect.y);
        maxX = Math.max(maxX, rect.x + rect.w);
        maxY = Math.max(maxY, rect.y + rect.h);
    }
    if (!Number.isFinite(minX) ||
        !Number.isFinite(minY) ||
        !Number.isFinite(maxX) ||
        !Number.isFinite(maxY)) {
        return null;
    }
    return { minX, minY, maxX, maxY };
}
/**
 * Get placement group label.
 */
export function getPlacementGroupLabel(defaultAlias, group, getGroupSugarcubes) {
    const meta = getGroupSugarcubes(group);
    const displayName = resolveInstanceDisplayName({
        metadata: meta,
        ...(group ? { group } : {}),
        fallback: defaultAlias,
    });
    return displayName || null;
}
function getPreviewLabelFontSize(scale) {
    return Math.max(10, 18 / scale);
}
/**
 * Draw ghost rect.
 */
export function drawGhostRect(ctx, rect, style, scale, label = null) {
    if (!rect) {
        return;
    }
    const lineWidth = Math.max(1, 2 / scale);
    const dash = [8 / scale, 5 / scale];
    ctx.save();
    ctx.lineWidth = lineWidth;
    ctx.setLineDash(dash);
    ctx.fillStyle = style.fill;
    ctx.strokeStyle = style.stroke;
    ctx.globalAlpha = style.alpha ?? 1;
    ctx.fillRect(rect.x, rect.y, rect.w, rect.h);
    ctx.strokeRect(rect.x, rect.y, rect.w, rect.h);
    if (label) {
        ctx.globalAlpha = 0.9;
        ctx.setLineDash([]);
        ctx.font = `${getPreviewLabelFontSize(scale)}px sans-serif`;
        ctx.fillStyle = style.stroke;
        ctx.fillText(label, rect.x + 6 / scale, rect.y + 14 / scale);
    }
    ctx.restore();
}
