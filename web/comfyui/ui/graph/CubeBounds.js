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
 * Own the SugarCubes graph integration layer in `web/comfyui/ui/graph/CubeBounds.js`.
 */
import { readGroupBounds, readNodeBounds } from './Bounds.js';
import { isRecord } from '../types/common.js';
/**
 * Expose the cube instance padding constant.
 */
export const CUBE_INSTANCE_PADDING = Object.freeze({ x: 2, y: 2 });
/**
 * Expose the cube instance top extra constant.
 */
export const CUBE_INSTANCE_TOP_EXTRA = 0;
/**
 * Expose the cube instance header height constant.
 */
export const CUBE_INSTANCE_HEADER_HEIGHT = 32;
/**
 * Expose the minimum visual margins for newly managed cube groups.
 */
export const CUBE_INSTANCE_AUTO_MIN_MARGINS = Object.freeze({
    left: 10,
    right: 10,
    bottom: 10,
    innerTop: 26,
});
const LEGACY_CUBE_INSTANCE_PADDING = Object.freeze({ x: 12, y: 12, top_extra: 12 });
function isFiniteNumber(value) {
    return Number.isFinite(Number(value));
}
function hasWritableVec2(value) {
    return ((Array.isArray(value) || ArrayBuffer.isView(value)) &&
        Number(value.length) >= 2);
}
function hasWritableBounds(value) {
    return ((Array.isArray(value) || ArrayBuffer.isView(value)) &&
        Number(value.length) >= 4);
}
function isCollapsedNode(node) {
    return isRecord(node?.flags) && node.flags.collapsed === true;
}
function readLiteGraphNumber(name, fallback) {
    const host = typeof LiteGraph !== 'undefined' ? LiteGraph : null;
    const value = Number(host?.[name]);
    return Number.isFinite(value) ? value : fallback;
}
function readNodeLayoutBounds(node) {
    if (!node) {
        return null;
    }
    const pos = node?.pos;
    const size = node?.size;
    const posReadable = (Array.isArray(pos) || ArrayBuffer.isView(pos)) && (pos?.length || 0) >= 2;
    const sizeReadable = (Array.isArray(size) || ArrayBuffer.isView(size)) && (size?.length || 0) >= 2;
    if (isCollapsedNode(node) && posReadable) {
        const x = Number(pos[0]);
        const yPos = Number(pos[1]);
        if (Number.isFinite(x) && Number.isFinite(yPos)) {
            const measured = readNodeBounds(node);
            const measuredW = Number(measured?.[2]);
            const measuredH = Number(measured?.[3]);
            const collapsedW = Number(node?._collapsed_width);
            const sizeW = Number(size?.[0]);
            const width = Number.isFinite(measuredW)
                ? measuredW
                : Number.isFinite(collapsedW)
                    ? collapsedW
                    : Number.isFinite(sizeW)
                        ? sizeW
                        : readLiteGraphNumber('NODE_COLLAPSED_WIDTH', 80);
            const height = Number.isFinite(measuredH)
                ? measuredH
                : readLiteGraphNumber('NODE_TITLE_HEIGHT', 30);
            const anchorY = readLiteGraphNumber('NODE_TITLE_HEIGHT', 30);
            return [x, yPos - anchorY, width, height];
        }
    }
    if (posReadable && sizeReadable) {
        const x = Number(pos[0]);
        const y = Number(pos[1]);
        const w = Number(size[0]);
        const h = Number(size[1]);
        if (Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(w) && Number.isFinite(h)) {
            return [x, y, w, h];
        }
    }
    return readNodeBounds(node);
}
function isValidBounds(bounds) {
    return (isRecord(bounds) &&
        isFiniteNumber(bounds.x) &&
        isFiniteNumber(bounds.y) &&
        isFiniteNumber(bounds.w) &&
        isFiniteNumber(bounds.h));
}
function normalizePadding(padding) {
    const source = isRecord(padding) ? padding : {};
    const padX = Number(source.x);
    const padY = Number(source.y);
    const topExtra = Number(source.top_extra);
    const resolved = {
        x: Number.isFinite(padX) ? padX : CUBE_INSTANCE_PADDING.x,
        y: Number.isFinite(padY) ? padY : CUBE_INSTANCE_PADDING.y,
        top_extra: Number.isFinite(topExtra) ? topExtra : CUBE_INSTANCE_TOP_EXTRA,
    };
    if (resolved.x === LEGACY_CUBE_INSTANCE_PADDING.x &&
        resolved.y === LEGACY_CUBE_INSTANCE_PADDING.y &&
        resolved.top_extra === LEGACY_CUBE_INSTANCE_PADDING.top_extra) {
        return {
            x: CUBE_INSTANCE_PADDING.x,
            y: CUBE_INSTANCE_PADDING.y,
            top_extra: CUBE_INSTANCE_TOP_EXTRA,
        };
    }
    return resolved;
}
function resolveHeaderHeight(header) {
    const headerHeight = Number(isRecord(header) ? header.height : undefined);
    return Number.isFinite(headerHeight) ? headerHeight : CUBE_INSTANCE_HEADER_HEIGHT;
}
function resolvePaddingHeader(bounds, metadata) {
    const boundsRecord = isRecord(bounds) ? bounds : {};
    const metadataBounds = isRecord(metadata?.bounds) ? metadata.bounds : {};
    const paddingSource = boundsRecord.padding ?? metadataBounds.padding ?? metadata?.padding ?? null;
    const headerSource = boundsRecord.header ?? metadataBounds.header ?? metadata?.header ?? null;
    const resolvedPadding = normalizePadding(paddingSource);
    const headerHeight = resolveHeaderHeight(headerSource);
    return {
        padding: resolvedPadding,
        header: { height: headerHeight },
    };
}
/**
 * Normalize bounds payload.
 */
export function normalizeBoundsPayload(bounds, metadata) {
    const resolved = resolvePaddingHeader(bounds, metadata);
    return {
        x: bounds.x,
        y: bounds.y,
        w: bounds.w,
        h: bounds.h,
        padding: {
            x: resolved.padding.x,
            y: resolved.padding.y,
            top_extra: resolved.padding.top_extra,
        },
        header: {
            height: resolved.header.height,
        },
    };
}
/**
 * Resolve canonical padding.
 */
export function resolveCanonicalPadding(metadata, bounds) {
    const resolved = resolvePaddingHeader(bounds ?? metadata?.bounds ?? null, metadata);
    return {
        padding: { ...resolved.padding },
        header: { ...resolved.header },
    };
}
/**
 * Compute instance bounds.
 */
export function computeInstanceBounds(nodes, markers) {
    const entries = [...(nodes || []), ...(markers || [])];
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of entries) {
        const bounds = readNodeLayoutBounds(node);
        if (!bounds) {
            continue;
        }
        const [x, y, w, h] = bounds;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x + w);
        maxY = Math.max(maxY, y + h);
    }
    if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
        return null;
    }
    return { minX, minY, maxX, maxY };
}
/**
 * Compute live visual content bounds.
 */
export function computeVisualContentBounds(nodes, markers) {
    const entries = [...(nodes || []), ...(markers || [])];
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of entries) {
        const rect = readNodeBounds(node);
        if (!rect || rect.length < 4) {
            continue;
        }
        const x = Number(rect[0]);
        const y = Number(rect[1]);
        const w = Number(rect[2]);
        const h = Number(rect[3]);
        if (![x, y, w, h].every(Number.isFinite)) {
            continue;
        }
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x + w);
        maxY = Math.max(maxY, y + h);
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
 * Inflate instance bounds.
 */
export function inflateInstanceBounds(bounds, padding = null) {
    if (!bounds) {
        return null;
    }
    const resolvedPadding = normalizePadding(padding);
    const padX = resolvedPadding.x;
    const padY = resolvedPadding.y;
    const padTop = padY + resolvedPadding.top_extra;
    const padBottom = padY;
    const header = resolveHeaderHeight(isRecord(padding) ? padding.header : null);
    return {
        x: bounds.minX - padX,
        y: bounds.minY - padTop - header,
        w: bounds.maxX - bounds.minX + padX * 2,
        h: bounds.maxY - bounds.minY + padTop + padBottom + header,
    };
}
function computeContentMargins(bounds, contentBounds) {
    if (!bounds || !contentBounds) {
        return null;
    }
    const left = contentBounds.minX - bounds.x;
    const top = contentBounds.minY - bounds.y;
    const right = bounds.x + bounds.w - contentBounds.maxX;
    const bottom = bounds.y + bounds.h - contentBounds.maxY;
    if (![left, top, right, bottom].every(Number.isFinite)) {
        return null;
    }
    return { left, top, right, bottom };
}
/**
 * Expand bounds to satisfy content margins.
 */
export function expandBoundsForContentMargins(bounds, contentBounds, minimumMargins) {
    if (!bounds || !contentBounds) {
        return bounds;
    }
    const current = computeContentMargins(bounds, contentBounds);
    if (!current) {
        return bounds;
    }
    const minLeft = Number(minimumMargins?.left);
    const minTop = Number(minimumMargins?.top);
    const minRight = Number(minimumMargins?.right);
    const minBottom = Number(minimumMargins?.bottom);
    const next = { ...bounds };
    const addLeft = (Number.isFinite(minLeft) ? minLeft : 0) - (contentBounds.minX - next.x);
    if (addLeft > 0) {
        next.x -= addLeft;
        next.w += addLeft;
    }
    const addTop = (Number.isFinite(minTop) ? minTop : 0) - (contentBounds.minY - next.y);
    if (addTop > 0) {
        next.y -= addTop;
        next.h += addTop;
    }
    const addRight = (Number.isFinite(minRight) ? minRight : 0) - (next.x + next.w - contentBounds.maxX);
    if (addRight > 0) {
        next.w += addRight;
    }
    const addBottom = (Number.isFinite(minBottom) ? minBottom : 0) - (next.y + next.h - contentBounds.maxY);
    if (addBottom > 0) {
        next.h += addBottom;
    }
    return next;
}
/**
 * Return whether content already fits within bounds.
 */
export function contentFitsWithinBounds(bounds, contentBounds, tolerance = 2) {
    if (!bounds || !contentBounds) {
        return false;
    }
    const slack = Number(tolerance);
    const resolvedSlack = Number.isFinite(slack) ? slack : 0;
    return (contentBounds.minX >= bounds.x - resolvedSlack &&
        contentBounds.minY >= bounds.y - resolvedSlack &&
        contentBounds.maxX <= bounds.x + bounds.w + resolvedSlack &&
        contentBounds.maxY <= bounds.y + bounds.h + resolvedSlack);
}
function unionContentBounds(primary, secondary) {
    if (!primary) {
        return secondary || null;
    }
    if (!secondary) {
        return primary;
    }
    return {
        minX: Math.min(primary.minX, secondary.minX),
        minY: Math.min(primary.minY, secondary.minY),
        maxX: Math.max(primary.maxX, secondary.maxX),
        maxY: Math.max(primary.maxY, secondary.maxY),
    };
}
/**
 * Resolve reusable cube chrome bounds from content.
 */
export function resolveChromeBoundsFromContent({ nodes, markers, padding, header, minimumMargins = CUBE_INSTANCE_AUTO_MIN_MARGINS, } = {}) {
    const contentBounds = computeInstanceBounds(nodes, markers);
    const visualContentBounds = computeVisualContentBounds(nodes, markers);
    const resolvedPadding = normalizePadding(padding);
    const resolvedHeader = { height: resolveHeaderHeight(header) };
    const chromeContentBounds = unionContentBounds(contentBounds, visualContentBounds);
    if (!chromeContentBounds) {
        return null;
    }
    const marginLeft = Number.isFinite(Number(minimumMargins.left)) ? Number(minimumMargins.left) : 0;
    const marginRight = Number.isFinite(Number(minimumMargins.right))
        ? Number(minimumMargins.right)
        : 0;
    const marginBottom = Number.isFinite(Number(minimumMargins.bottom))
        ? Number(minimumMargins.bottom)
        : 0;
    const innerTop = Number.isFinite(Number(minimumMargins.innerTop))
        ? Number(minimumMargins.innerTop)
        : 0;
    const marginTop = resolvedPadding.y + resolvedPadding.top_extra + resolvedHeader.height + innerTop;
    const resolvedBounds = {
        x: chromeContentBounds.minX - marginLeft,
        y: chromeContentBounds.minY - marginTop,
        w: chromeContentBounds.maxX - chromeContentBounds.minX + marginLeft + marginRight,
        h: chromeContentBounds.maxY - chromeContentBounds.minY + marginTop + marginBottom,
    };
    return normalizeBoundsPayload({
        ...resolvedBounds,
        padding: { ...resolvedPadding },
        header: { ...resolvedHeader },
    }, null);
}
/**
 * Resolve bounds for a newly managed cube group.
 */
export function resolveNewInstanceBounds(options = {}) {
    return resolveChromeBoundsFromContent(options);
}
/**
 * Resolve instance bounds.
 */
export function resolveInstanceBounds({ group, metadata, nodes, markers, } = {}) {
    const metaBounds = metadata?.bounds;
    if (isValidBounds(metaBounds)) {
        return normalizeBoundsPayload(metaBounds, metadata);
    }
    const groupBounds = readGroupBounds(group);
    if (groupBounds) {
        return normalizeBoundsPayload({
            x: groupBounds[0],
            y: groupBounds[1],
            w: groupBounds[2],
            h: groupBounds[3],
        }, metadata);
    }
    const computed = computeInstanceBounds(nodes, markers);
    if (!computed) {
        return null;
    }
    return inflateInstanceBounds(computed, CUBE_INSTANCE_PADDING);
}
/**
 * Compute inner bounds.
 */
export function computeInnerBounds(bounds) {
    if (!bounds || !isValidBounds(bounds)) {
        return null;
    }
    const { padding, header } = resolvePaddingHeader(bounds, null);
    const padX = padding.x;
    const padY = padding.y;
    const topExtra = padding.top_extra;
    const headerHeight = header.height;
    const innerX = bounds.x + padX;
    const innerY = bounds.y + padY + topExtra + headerHeight;
    const innerW = bounds.w - padX * 2;
    const innerH = bounds.h - (padY * 2 + topExtra + headerHeight);
    return {
        x: innerX,
        y: innerY,
        w: innerW,
        h: innerH,
    };
}
/**
 * Expand bounds to include rect.
 */
export function expandBoundsToIncludeRect(bounds, rect, extraPadding = 2) {
    if (!bounds || !rect || !isValidBounds(bounds)) {
        return bounds || null;
    }
    const pad = Number(extraPadding);
    const expandBy = Number.isFinite(pad) ? pad : 2;
    const rectX = Number(rect.x);
    const rectY = Number(rect.y);
    const rectW = Number(rect.w);
    const rectH = Number(rect.h);
    if (!Number.isFinite(rectX) || !Number.isFinite(rectY)) {
        return bounds;
    }
    const width = Number.isFinite(rectW) ? rectW : 0;
    const height = Number.isFinite(rectH) ? rectH : 0;
    const minX = Math.min(bounds.x, rectX - expandBy);
    const minY = Math.min(bounds.y, rectY - expandBy);
    const maxX = Math.max(bounds.x + bounds.w, rectX + width + expandBy);
    const maxY = Math.max(bounds.y + bounds.h, rectY + height + expandBy);
    return normalizeBoundsPayload({
        x: minX,
        y: minY,
        w: maxX - minX,
        h: maxY - minY,
    }, { bounds });
}
/**
 * Write canonical bounds.
 */
export function writeCanonicalBounds({ group, metadata, bounds, } = {}) {
    if (!bounds) {
        return null;
    }
    const rawTargetMetadata = metadata ?? group?.properties?.sugarcubes;
    if (rawTargetMetadata != null && !isRecord(rawTargetMetadata)) {
        return null;
    }
    const targetMetadata = rawTargetMetadata ?? null;
    const payload = normalizeBoundsPayload(bounds, targetMetadata || { bounds });
    if (targetMetadata) {
        targetMetadata.bounds = payload;
    }
    if (group) {
        if (!group.properties || typeof group.properties !== 'object') {
            group.properties = {};
        }
        if (targetMetadata) {
            targetMetadata.bounds = payload;
            group.properties.sugarcubes = targetMetadata;
        }
        if (hasWritableVec2(group.pos)) {
            group.pos[0] = payload.x;
            group.pos[1] = payload.y;
        }
        else {
            group.pos = [payload.x, payload.y];
        }
        if (hasWritableVec2(group.size)) {
            group.size[0] = payload.w;
            group.size[1] = payload.h;
        }
        else {
            group.size = [payload.w, payload.h];
        }
        if (hasWritableBounds(group._bounding)) {
            group._bounding[0] = payload.x;
            group._bounding[1] = payload.y;
            group._bounding[2] = payload.w;
            group._bounding[3] = payload.h;
        }
    }
    return payload;
}
