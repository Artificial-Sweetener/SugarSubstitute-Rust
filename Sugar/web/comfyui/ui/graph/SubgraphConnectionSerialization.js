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
/** Normalize Comfy subgraph connection primitives without changing identity. */
/** Normalize object- and tuple-shaped Comfy links while preserving node ids. */
export function normalizeSubgraphLinks(rawLinks) {
    if (!Array.isArray(rawLinks))
        return [];
    return rawLinks.map((rawLink, index) => {
        const normalized = normalizeSubgraphLink(rawLink);
        if (normalized === null) {
            throw new TypeError(`Comfy subgraph serialized link at index ${index} is invalid`);
        }
        return normalized;
    });
}
/** Normalize optional serialized slot type values. */
export function normalizeSubgraphSlotType(value) {
    return typeof value === 'string' && value.trim() ? value.trim() : '*';
}
/** Validate one serialized link without inventing topology values. */
function normalizeSubgraphLink(rawLink) {
    const values = readLinkValues(rawLink);
    if (!values)
        return null;
    const id = readInteger(values.id);
    const originId = readSerializedGraphId(values.originId);
    const originSlot = readNonnegativeInteger(values.originSlot);
    const targetId = readSerializedGraphId(values.targetId);
    const targetSlot = readNonnegativeInteger(values.targetSlot);
    if (id === null ||
        originId === null ||
        originSlot === null ||
        targetId === null ||
        targetSlot === null) {
        return null;
    }
    const parentId = values.parentId == null ? null : readInteger(values.parentId);
    if (values.parentId != null && parentId === null)
        return null;
    return {
        id,
        origin_id: originId,
        origin_slot: originSlot,
        target_id: targetId,
        target_slot: targetSlot,
        type: normalizeSubgraphSlotType(values.type),
        ...(parentId !== null ? { parentId } : {}),
    };
}
/** Project supported host link shapes into one validation record. */
function readLinkValues(rawLink) {
    if (Array.isArray(rawLink)) {
        if (rawLink.length < 5)
            return null;
        const [id, originId, originSlot, targetId, targetSlot, type, parentId] = rawLink;
        return { id, originId, originSlot, targetId, targetSlot, type, parentId };
    }
    if (!isRecord(rawLink))
        return null;
    return {
        id: rawLink.id,
        originId: rawLink.origin_id,
        originSlot: rawLink.origin_slot,
        targetId: rawLink.target_id,
        targetSlot: rawLink.target_slot,
        type: rawLink.type,
        parentId: rawLink.parentId,
    };
}
/** Read Comfy's authoritative number-or-string serialized node identity. */
export function readSerializedGraphId(value) {
    if (typeof value === 'string')
        return value.length > 0 ? value : null;
    return readInteger(value);
}
/** Read one exact integer without boolean or string coercion. */
function readInteger(value) {
    return typeof value === 'number' && Number.isInteger(value) ? value : null;
}
/** Read one exact nonnegative slot index. */
function readNonnegativeInteger(value) {
    const result = readInteger(value);
    return result !== null && result >= 0 ? result : null;
}
/** Narrow one dynamic host object without accepting arrays. */
function isRecord(value) {
    return typeof value === 'object' && value !== null && !Array.isArray(value);
}
