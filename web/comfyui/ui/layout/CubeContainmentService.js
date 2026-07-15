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
 * Own the SugarCubes layout orchestration layer in `web/comfyui/ui/layout/CubeContainmentService.js`.
 */
import { readNodeBounds } from '../graph/Bounds.js';
import { computeInnerBounds, expandBoundsToIncludeRect, writeCanonicalBounds, } from '../graph/CubeBounds.js';
import { isCubeMarkerType } from '../graph/CubeMarkers.js';
import { CubeInstanceIndex } from './CubeInstanceIndex.js';
import { isRecord } from '../types/common.js';
function clampAxis(value, min, max) {
    const low = Math.min(min, max);
    const high = Math.max(min, max);
    return Math.min(Math.max(value, low), high);
}
function isCollapsedNode(node) {
    return isRecord(node?.flags) && node.flags.collapsed === true;
}
function readLiteGraphNumber(name, fallback) {
    const liteGraph = typeof LiteGraph !== 'undefined' ? LiteGraph : null;
    const value = Number(liteGraph?.[name]);
    return Number.isFinite(value) ? value : fallback;
}
function isReadableNumericArray(value, minimumLength) {
    return ((Array.isArray(value) || ArrayBuffer.isView(value)) &&
        Number(value.length) >= minimumLength);
}
function rectFromNode(node) {
    const pos = node?.pos;
    const size = node?.size;
    const posReadable = isReadableNumericArray(pos, 2);
    const sizeReadable = isReadableNumericArray(size, 2);
    if (isCollapsedNode(node)) {
        if (posReadable) {
            const x = Number(pos[0]);
            const yPos = Number(pos[1]);
            if (Number.isFinite(x) && Number.isFinite(yPos)) {
                const bounds = readNodeBounds(node);
                const measuredW = Number(bounds?.[2]);
                const measuredH = Number(bounds?.[3]);
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
                return {
                    x,
                    y: yPos - anchorY,
                    w: width,
                    h: height,
                    anchorX: 0,
                    anchorY,
                };
            }
        }
    }
    if (posReadable && sizeReadable) {
        const x = Number(pos[0]);
        const y = Number(pos[1]);
        const w = Number(size[0]);
        const h = Number(size[1]);
        if (Number.isFinite(x) && Number.isFinite(y) && Number.isFinite(w) && Number.isFinite(h)) {
            return { x, y, w, h, anchorX: 0, anchorY: 0 };
        }
    }
    const bounds = readNodeBounds(node);
    if (!bounds) {
        return null;
    }
    const [x, y, w, h] = bounds;
    return { x, y, w, h, anchorX: 0, anchorY: 0 };
}
function rectOutsideBounds(rect, bounds) {
    const rectRight = rect.x + rect.w;
    const rectBottom = rect.y + rect.h;
    const boundsRight = bounds.x + bounds.w;
    const boundsBottom = bounds.y + bounds.h;
    return (rect.x < bounds.x || rect.y < bounds.y || rectRight > boundsRight || rectBottom > boundsBottom);
}
function updateNodePosition(node, x, y, rect) {
    const pos = node?.pos;
    const posWritable = isReadableNumericArray(pos, 2);
    if (!posWritable) {
        return false;
    }
    const nextX = Number(x);
    const nextY = Number(y);
    if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) {
        return false;
    }
    const anchorX = Number(rect?.anchorX);
    const anchorY = Number(rect?.anchorY);
    const offsetX = Number.isFinite(anchorX) ? anchorX : 0;
    const offsetY = Number.isFinite(anchorY) ? anchorY : 0;
    pos[0] = nextX + offsetX;
    pos[1] = nextY + offsetY;
    return true;
}
function resolveIndexedInstanceBounds(instance) {
    const metadataBounds = instance.metadata?.bounds;
    return isRectBounds(metadataBounds) ? metadataBounds : instance.bounds;
}
function areBoundsEqual(a, b) {
    if (!a || !b) {
        return false;
    }
    return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
}
/**
 * Coordinate cube containment service behavior for the SugarCubes UI.
 */
export class CubeContainmentService {
    indexFactory;
    constructor({ indexFactory } = {}) {
        this.indexFactory = indexFactory ?? null;
    }
    buildIndex(graph) {
        if (this.indexFactory) {
            return this.indexFactory(graph);
        }
        return new CubeInstanceIndex(graph === undefined ? {} : { graph });
    }
    enforceForNodes({ graph, nodes, index } = {}) {
        const movedNodes = Array.isArray(nodes) ? nodes.filter(Boolean) : [];
        if (!movedNodes.length) {
            return { clamped: 0, expanded: 0, instances: new Set() };
        }
        const indexRef = index || this.buildIndex(graph);
        const instanceGroups = new Map();
        for (const node of movedNodes) {
            const nodeId = node?.id != null ? String(node.id) : '';
            if (!nodeId) {
                continue;
            }
            const instance = isCubeMarkerType(node)
                ? indexRef?.instanceByMarkerId?.get?.(nodeId)
                : indexRef?.instanceByNodeId?.get?.(nodeId);
            if (!instance) {
                continue;
            }
            const bucket = instanceGroups.get(instance.instanceId) || { instance, nodes: [] };
            bucket.nodes.push(node);
            instanceGroups.set(instance.instanceId, bucket);
        }
        let clamped = 0;
        let expanded = 0;
        const touched = new Set();
        for (const entry of instanceGroups.values()) {
            const instance = entry.instance;
            const bounds = resolveIndexedInstanceBounds(instance);
            if (!bounds) {
                continue;
            }
            const inner = computeInnerBounds(bounds);
            if (!inner) {
                continue;
            }
            let nextBounds = bounds;
            let didExpand = false;
            for (const node of entry.nodes) {
                const rect = rectFromNode(node);
                if (!rect) {
                    continue;
                }
                if (isCubeMarkerType(node)) {
                    if (rectOutsideBounds(rect, inner)) {
                        const updated = expandBoundsToIncludeRect(nextBounds, rect, 2);
                        if (updated && !areBoundsEqual(updated, nextBounds)) {
                            nextBounds = updated;
                            didExpand = true;
                        }
                    }
                    continue;
                }
                const maxX = inner.x + inner.w - rect.w;
                const maxY = inner.y + inner.h - rect.h;
                const clampedX = clampAxis(rect.x, inner.x, maxX);
                const clampedY = clampAxis(rect.y, inner.y, maxY);
                if (clampedX !== rect.x || clampedY !== rect.y) {
                    if (updateNodePosition(node, clampedX, clampedY, rect)) {
                        clamped += 1;
                    }
                }
            }
            if (didExpand) {
                writeCanonicalBounds({
                    group: instance.group,
                    metadata: instance.metadata,
                    bounds: nextBounds,
                });
                instance.bounds = { ...nextBounds };
                expanded += 1;
            }
            touched.add(instance.instanceId);
        }
        return { clamped, expanded, instances: touched };
    }
}
function isRectBounds(value) {
    if (!isRecord(value)) {
        return false;
    }
    return [value.x, value.y, value.w, value.h].every((entry) => Number.isFinite(Number(entry)));
}
