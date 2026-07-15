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
 * Own the SugarCubes subgraph serialization compatibility helpers in
 * `frontend/comfyui/ui/graph/SubgraphSerialization.js`.
 */
import { isRecord } from '../types/common.js';
const SUBGRAPH_INPUT_ID = -10;
const SUBGRAPH_OUTPUT_ID = -20;
const DEFAULT_IO_BOUNDS = Object.freeze([0, 0, 75, 100]);
const DEFAULT_NODE_SIZE = Object.freeze([180, 60]);
/**
 * Normalize one subgraph payload into the current LiteGraph-compatible shape.
 */
export function normalizeSubgraphPayload(rawEntry, fallbackId, options = {}) {
    if (!isRecord(rawEntry) || Array.isArray(rawEntry)) {
        return null;
    }
    const serialized = deepClone(rawEntry);
    const subgraphId = readTrimmedString(serialized.id) || readTrimmedString(fallbackId);
    if (!subgraphId) {
        return null;
    }
    return looksLikeCurrentSubgraphPayload(serialized)
        ? normalizeCurrentSubgraphPayload(serialized, subgraphId, options)
        : normalizeLegacySubgraphPayload(serialized, subgraphId, options);
}
/**
 * Return whether a payload already exposes current subgraph-only metadata.
 */
export function looksLikeCurrentSubgraphPayload(entry) {
    if (!isRecord(entry) || Array.isArray(entry)) {
        return false;
    }
    return Boolean((entry.inputNode && typeof entry.inputNode === 'object') ||
        (entry.outputNode && typeof entry.outputNode === 'object') ||
        typeof entry.name === 'string');
}
/**
 * Return whether a payload matches the older workflow-style subgraph shape.
 */
export function looksLikeLegacyWorkflowSubgraph(entry) {
    if (!isRecord(entry) || Array.isArray(entry)) {
        return false;
    }
    return (!looksLikeCurrentSubgraphPayload(entry) &&
        Array.isArray(entry.nodes) &&
        Array.isArray(entry.links));
}
/**
 * Normalize one current-format subgraph payload.
 */
function normalizeCurrentSubgraphPayload(entry, subgraphId, options) {
    const nodes = normalizeNodeEntries(entry.nodes);
    const links = normalizeLinkEntries(entry.links);
    const groups = normalizeObjectArray(entry.groups);
    const reroutes = normalizeObjectArray(entry.reroutes);
    const floatingLinks = normalizeObjectArray(entry.floatingLinks);
    const state = normalizeGraphState(entry, nodes, links, groups, reroutes);
    const layoutBounds = computeGraphBounds(nodes, groups);
    const fallbackName = readTrimmedString(options.fallbackName) || subgraphId;
    const inputs = Array.isArray(entry.inputs) && entry.inputs.length
        ? normalizeSubgraphIoArray(entry.inputs, 'input', subgraphId)
        : buildLegacySubgraphInputs(links, nodes, subgraphId, options.expectedInputNames);
    const outputs = Array.isArray(entry.outputs) && entry.outputs.length
        ? normalizeSubgraphIoArray(entry.outputs, 'output', subgraphId)
        : buildLegacySubgraphOutputs(links, nodes, subgraphId);
    return {
        id: subgraphId,
        version: 1,
        revision: coerceInteger(entry.revision, 0),
        state,
        config: normalizeObject(entry.config),
        name: readTrimmedString(entry.name) || fallbackName,
        inputNode: normalizeIoNode(entry.inputNode, SUBGRAPH_INPUT_ID, layoutBounds, 'input'),
        outputNode: normalizeIoNode(entry.outputNode, SUBGRAPH_OUTPUT_ID, layoutBounds, 'output'),
        inputs,
        outputs,
        widgets: normalizeObjectArray(entry.widgets),
        nodes,
        links,
        floatingLinks,
        reroutes,
        groups,
        extra: normalizeObject(entry.extra),
    };
}
/**
 * Normalize one legacy workflow-style subgraph payload.
 */
function normalizeLegacySubgraphPayload(entry, subgraphId, options) {
    const nodes = normalizeNodeEntries(entry.nodes);
    const links = normalizeLinkEntries(entry.links);
    const groups = normalizeObjectArray(entry.groups);
    const reroutes = normalizeObjectArray(entry.reroutes);
    const layoutBounds = computeGraphBounds(nodes, groups);
    return {
        id: subgraphId,
        version: 1,
        revision: coerceInteger(entry.revision, 0),
        state: normalizeGraphState(entry, nodes, links, groups, reroutes),
        config: normalizeObject(entry.config),
        name: readTrimmedString(entry.name) || readTrimmedString(options.fallbackName) || subgraphId,
        inputNode: normalizeIoNode(null, SUBGRAPH_INPUT_ID, layoutBounds, 'input'),
        outputNode: normalizeIoNode(null, SUBGRAPH_OUTPUT_ID, layoutBounds, 'output'),
        inputs: buildLegacySubgraphInputs(links, nodes, subgraphId, options.expectedInputNames),
        outputs: buildLegacySubgraphOutputs(links, nodes, subgraphId),
        widgets: [],
        nodes,
        links,
        floatingLinks: normalizeObjectArray(entry.floatingLinks),
        reroutes,
        groups,
        extra: normalizeObject(entry.extra),
    };
}
/**
 * Normalize persisted graph state counters.
 */
function normalizeGraphState(entry, nodes, links, groups, reroutes) {
    const rawState = isRecord(entry.state) ? entry.state : {};
    return {
        lastNodeId: Math.max(coerceInteger(rawState.lastNodeId, 0), coerceInteger(entry.last_node_id, 0), maxNumericValue(nodes.map((node) => node.id))),
        lastLinkId: Math.max(coerceInteger(rawState.lastLinkId, 0), coerceInteger(entry.last_link_id, 0), maxNumericValue(links.map((link) => link.id))),
        lastGroupId: Math.max(coerceInteger(rawState.lastGroupId, 0), maxNumericValue(groups.map((group) => group.id))),
        lastRerouteId: Math.max(coerceInteger(rawState.lastRerouteId, 0), maxNumericValue(reroutes.map((reroute) => reroute.id))),
    };
}
/**
 * Normalize one IO node definition.
 */
function normalizeIoNode(rawNode, defaultId, layoutBounds, side) {
    const node = isRecord(rawNode) ? rawNode : {};
    const bounding = normalizeBounding(node.bounding, layoutBounds, side);
    return {
        id: defaultId,
        bounding,
        pinned: Boolean(node.pinned),
    };
}
/**
 * Normalize one bounding box.
 */
function normalizeBounding(rawBounding, layoutBounds, side) {
    if (Array.isArray(rawBounding) && rawBounding.length === 4) {
        return rawBounding.map((value, index) => index < 2
            ? coerceNumber(value, 0)
            : Math.max(0, coerceNumber(value, DEFAULT_IO_BOUNDS[index] ?? 0)));
    }
    if (!layoutBounds) {
        return [...DEFAULT_IO_BOUNDS];
    }
    const width = DEFAULT_IO_BOUNDS[2];
    const height = DEFAULT_IO_BOUNDS[3];
    const centerY = layoutBounds.minY + layoutBounds.height * 0.5 - height * 0.5;
    const x = side === 'input' ? layoutBounds.minX - width - 50 : layoutBounds.maxX + 50;
    return [x, centerY, width, height];
}
/**
 * Normalize node entries from saved payloads.
 */
function normalizeNodeEntries(rawNodes) {
    if (!Array.isArray(rawNodes)) {
        return [];
    }
    return rawNodes
        .filter((node) => isRecord(node) && !Array.isArray(node))
        .map((node) => deepClone(node));
}
/**
 * Normalize link entries into object-shaped LiteGraph links.
 */
function normalizeLinkEntries(rawLinks) {
    if (!Array.isArray(rawLinks)) {
        return [];
    }
    return rawLinks
        .map((link, index) => normalizeLinkEntry(link, index))
        .filter((link) => link !== null);
}
/**
 * Normalize one serialized link entry.
 */
function normalizeLinkEntry(rawLink, index) {
    if (Array.isArray(rawLink)) {
        const [id, originId, originSlot, targetId, targetSlot, type, parentId] = rawLink;
        return {
            id: coerceInteger(id, index + 1),
            origin_id: coerceInteger(originId, 0),
            origin_slot: coerceInteger(originSlot, 0),
            target_id: coerceInteger(targetId, 0),
            target_slot: coerceInteger(targetSlot, 0),
            type: normalizeSlotType(type),
            ...(parentId != null ? { parentId: coerceInteger(parentId, 0) } : {}),
        };
    }
    if (!isRecord(rawLink)) {
        return null;
    }
    return {
        id: coerceInteger(rawLink.id, index + 1),
        origin_id: coerceInteger(rawLink.origin_id, 0),
        origin_slot: coerceInteger(rawLink.origin_slot, 0),
        target_id: coerceInteger(rawLink.target_id, 0),
        target_slot: coerceInteger(rawLink.target_slot, 0),
        type: normalizeSlotType(rawLink.type),
        ...(rawLink.parentId != null ? { parentId: coerceInteger(rawLink.parentId, 0) } : {}),
    };
}
/**
 * Normalize an array of current subgraph IO entries.
 */
function normalizeSubgraphIoArray(entries, kind, subgraphId) {
    if (!Array.isArray(entries)) {
        return [];
    }
    const seenNames = new Set();
    return entries
        .filter((entry) => isRecord(entry) && !Array.isArray(entry))
        .map((entry, index) => normalizeSubgraphIoEntry(entry, index, kind, subgraphId, seenNames))
        .filter(Boolean);
}
/**
 * Normalize one current subgraph IO entry.
 */
function normalizeSubgraphIoEntry(entry, index, kind, subgraphId, seenNames) {
    const fallbackName = `${kind}_${index + 1}`;
    const name = makeUniqueName(readTrimmedString(entry.name) || fallbackName, seenNames);
    const label = resolveDisplayLabel(entry, name);
    return {
        id: readTrimmedString(entry.id) || `${subgraphId}:${kind}:${index}`,
        type: normalizeSlotType(entry.type),
        linkIds: normalizeLinkIdArray(entry.linkIds),
        name,
        label,
        ...(readOptionalString(entry.localized_name)
            ? { localized_name: readOptionalString(entry.localized_name) }
            : {}),
        ...(entry.shape != null ? { shape: entry.shape } : {}),
        ...(entry.color_off != null ? { color_off: entry.color_off } : {}),
        ...(entry.color_on != null ? { color_on: entry.color_on } : {}),
        ...(entry.dir != null ? { dir: entry.dir } : {}),
        ...(entry.hasErrors != null ? { hasErrors: Boolean(entry.hasErrors) } : {}),
    };
}
/**
 * Reconstruct current subgraph inputs from legacy boundary links.
 */
function buildLegacySubgraphInputs(links, nodes, subgraphId, expectedNames = []) {
    const grouped = groupLinksBySlot(links.filter((link) => link.origin_id === SUBGRAPH_INPUT_ID), 'origin_slot');
    return buildLegacySubgraphIo(grouped, nodes, subgraphId, 'input', expectedNames);
}
/**
 * Reconstruct current subgraph outputs from legacy boundary links.
 */
function buildLegacySubgraphOutputs(links, nodes, subgraphId) {
    const grouped = groupLinksBySlot(links.filter((link) => link.target_id === SUBGRAPH_OUTPUT_ID), 'target_slot');
    return buildLegacySubgraphIo(grouped, nodes, subgraphId, 'output');
}
/**
 * Build current subgraph IO entries from grouped legacy boundary links.
 */
function buildLegacySubgraphIo(groupedLinks, nodes, subgraphId, kind, expectedNames = []) {
    const seenNames = new Set();
    const nodeIndex = new Map(nodes.map((node) => [coerceInteger(node.id, Number.NaN), node]));
    const ioEntries = [];
    for (const [slotIndex, slotLinks] of groupedLinks.entries()) {
        const slotMeta = resolveLegacyBoundarySlot(slotLinks[0], nodeIndex, kind);
        const fallbackName = `${kind}_${slotIndex + 1}`;
        const name = makeUniqueName(readTrimmedString(expectedNames?.[slotIndex]) ||
            readTrimmedString(slotMeta?.name) ||
            fallbackName, seenNames);
        const label = resolveDisplayLabel(slotMeta, name);
        const localizedName = readOptionalString(slotMeta?.localized_name);
        ioEntries.push({
            id: `${subgraphId}:${kind}:${slotIndex}`,
            type: normalizeSlotType(slotMeta?.type),
            linkIds: slotLinks.map((link) => link.id),
            name,
            label,
            ...(localizedName ? { localized_name: localizedName } : {}),
            ...(slotMeta?.shape != null ? { shape: slotMeta.shape } : {}),
            ...(slotMeta?.color_off != null ? { color_off: slotMeta.color_off } : {}),
            ...(slotMeta?.color_on != null ? { color_on: slotMeta.color_on } : {}),
            ...(slotMeta?.dir != null ? { dir: slotMeta.dir } : {}),
            ...(slotMeta?.hasErrors != null ? { hasErrors: Boolean(slotMeta.hasErrors) } : {}),
        });
    }
    return ioEntries;
}
/**
 * Resolve legacy slot metadata from one boundary link.
 */
function resolveLegacyBoundarySlot(link, nodeIndex, kind) {
    if (!link) {
        return null;
    }
    if (kind === 'input') {
        const node = nodeIndex.get(link.target_id);
        const slot = Array.isArray(node?.inputs)
            ? node.inputs[coerceInteger(link.target_slot, -1)]
            : null;
        return isRecord(slot) ? slot : null;
    }
    const node = nodeIndex.get(link.origin_id);
    const slot = Array.isArray(node?.outputs)
        ? node.outputs[coerceInteger(link.origin_slot, -1)]
        : null;
    return isRecord(slot) ? slot : null;
}
/**
 * Compute graph bounds from serialized nodes and groups.
 */
function computeGraphBounds(nodes, groups) {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const node of nodes) {
        const pos = Array.isArray(node?.pos) ? node.pos : null;
        const size = Array.isArray(node?.size) ? node.size : DEFAULT_NODE_SIZE;
        if (!pos || pos.length < 2) {
            continue;
        }
        const x = coerceNumber(pos[0], 0);
        const y = coerceNumber(pos[1], 0);
        const width = Math.max(0, coerceNumber(size[0], DEFAULT_NODE_SIZE[0]));
        const height = Math.max(0, coerceNumber(size[1], DEFAULT_NODE_SIZE[1]));
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x + width);
        maxY = Math.max(maxY, y + height);
    }
    for (const group of groups) {
        const bounding = Array.isArray(group?.bounding) ? group.bounding : null;
        if (!bounding || bounding.length < 4) {
            continue;
        }
        const x = coerceNumber(bounding[0], 0);
        const y = coerceNumber(bounding[1], 0);
        const width = Math.max(0, coerceNumber(bounding[2], 0));
        const height = Math.max(0, coerceNumber(bounding[3], 0));
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x + width);
        maxY = Math.max(maxY, y + height);
    }
    if (!Number.isFinite(minX) ||
        !Number.isFinite(minY) ||
        !Number.isFinite(maxX) ||
        !Number.isFinite(maxY)) {
        return null;
    }
    return {
        minX,
        minY,
        maxX,
        maxY,
        width: maxX - minX,
        height: maxY - minY,
    };
}
/**
 * Group links by one slot property.
 */
function groupLinksBySlot(links, key) {
    const grouped = new Map();
    for (const link of links) {
        const slot = coerceInteger(link?.[key], -1);
        if (slot < 0) {
            continue;
        }
        const bucket = grouped.get(slot) || [];
        bucket.push(link);
        grouped.set(slot, bucket);
    }
    return new Map(Array.from(grouped.entries()).sort((a, b) => a[0] - b[0]));
}
/**
 * Normalize object arrays.
 */
function normalizeObjectArray(entries) {
    if (!Array.isArray(entries)) {
        return [];
    }
    return entries
        .filter((entry) => isRecord(entry) && !Array.isArray(entry))
        .map((entry) => deepClone(entry));
}
/**
 * Normalize link id arrays.
 */
function normalizeLinkIdArray(linkIds) {
    if (!Array.isArray(linkIds)) {
        return [];
    }
    return linkIds.map((linkId, index) => coerceInteger(linkId, index));
}
/**
 * Normalize optional slot type values.
 */
function normalizeSlotType(value) {
    if (typeof value === 'string' && value.trim()) {
        return value.trim();
    }
    return '*';
}
/**
 * Normalize plain object values.
 */
function normalizeObject(value) {
    return isRecord(value) && !Array.isArray(value) ? deepClone(value) : {};
}
/**
 * Generate a unique display name while preserving the original seed where possible.
 */
function makeUniqueName(seed, seen) {
    const base = readTrimmedString(seed) || 'slot';
    let candidate = base;
    let suffix = 2;
    while (seen.has(candidate)) {
        candidate = `${base}_${suffix}`;
        suffix += 1;
    }
    seen.add(candidate);
    return candidate;
}
/**
 * Return a deep clone safe for plain serialized payloads.
 */
function deepClone(value) {
    return JSON.parse(JSON.stringify(value));
}
/**
 * Read one trimmed string field.
 */
function readTrimmedString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read one optional string field without forcing non-empty output.
 */
function readOptionalString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Resolve Comfy's visible name for one public subgraph interface slot.
 */
function resolveDisplayLabel(entry, fallbackName) {
    return (readOptionalString(entry?.label) ||
        readOptionalString(entry?.localized_name) ||
        readTrimmedString(fallbackName));
}
/**
 * Convert a value into a finite number.
 */
function coerceNumber(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) ? number : fallback;
}
/**
 * Convert a value into an integer.
 */
function coerceInteger(value, fallback) {
    const number = Number(value);
    return Number.isInteger(number) ? number : fallback;
}
/**
 * Return the highest numeric value in a list.
 */
function maxNumericValue(values) {
    let max = 0;
    for (const value of values) {
        const number = Number(value);
        if (Number.isFinite(number) && number > max) {
            max = number;
        }
    }
    return max;
}
