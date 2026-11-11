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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/Bounds.js`.
 */
import { isRecord } from '../types/common.js';
function isNumericSequence(value, minimumLength) {
    if (Array.isArray(value)) {
        return value.length >= minimumLength && value.every((entry) => typeof entry === 'number');
    }
    return (ArrayBuffer.isView(value) &&
        'length' in value &&
        typeof value.length === 'number' &&
        value.length >= minimumLength);
}
/**
 * Read node bounds.
 */
export function readNodeBounds(node) {
    if (!isRecord(node))
        return null;
    if (typeof node.getBounding === 'function') {
        try {
            const bounds = node.getBounding();
            if (isNumericSequence(bounds, 4)) {
                return [bounds[0] ?? 0, bounds[1] ?? 0, bounds[2] ?? 0, bounds[3] ?? 0];
            }
        }
        catch (_error) {
            return null;
        }
    }
    const pos = node?.pos;
    const size = node?.size;
    const posReadable = isNumericSequence(pos, 2);
    const sizeReadable = isNumericSequence(size, 2);
    if (posReadable && sizeReadable) {
        return [pos[0] ?? 0, pos[1] ?? 0, size[0] ?? 0, size[1] ?? 0];
    }
    return null;
}
/**
 * Read group bounds.
 */
export function readGroupBounds(group) {
    if (!isRecord(group))
        return null;
    const pos = group.pos;
    const size = group.size;
    if (isNumericSequence(pos, 2) && isNumericSequence(size, 2)) {
        return [pos[0] ?? 0, pos[1] ?? 0, size[0] ?? 0, size[1] ?? 0];
    }
    const bounding = group._bounding;
    if (isNumericSequence(bounding, 4)) {
        return [bounding[0] ?? 0, bounding[1] ?? 0, bounding[2] ?? 0, bounding[3] ?? 0];
    }
    return null;
}
/**
 * Return whether point in bounds.
 */
export function isPointInBounds(point, bounds) {
    if (!point || !bounds)
        return false;
    const [x, y] = point;
    const [bx, by, bw, bh] = bounds;
    return x >= bx && x <= bx + bw && y >= by && y <= by + bh;
}
/**
 * Get node center.
 */
export function getNodeCenter(node) {
    if (isRecord(node) && isNumericSequence(node.pos, 2) && isNumericSequence(node.size, 2)) {
        return [
            (node.pos[0] ?? 0) + (node.size[0] ?? 0) / 2,
            (node.pos[1] ?? 0) + (node.size[1] ?? 0) / 2,
        ];
    }
    const bounds = readNodeBounds(node);
    if (!bounds)
        return null;
    return [bounds[0] + bounds[2] / 2, bounds[1] + bounds[3] / 2];
}
