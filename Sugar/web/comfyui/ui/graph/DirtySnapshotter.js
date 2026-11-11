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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtySnapshotter.js`.
 */
import { collectGraphLinks, getGraphNodes } from './GraphQuery.js';
import { readNodeBounds } from './Bounds.js';
import { isRecord } from '../types/common.js';
function isPlainObject(value) {
    if (!value || typeof value !== 'object') {
        return false;
    }
    const proto = Object.getPrototypeOf(value);
    return proto === Object.prototype || proto === null;
}
/**
 * Sanitize value.
 */
export function sanitizeValue(value, seen, depth) {
    if (value == null) {
        return null;
    }
    if (depth > 4) {
        return null;
    }
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
        return value;
    }
    if (typeof value === 'function') {
        return null;
    }
    if (typeof value === 'bigint') {
        return value.toString();
    }
    if (Array.isArray(value)) {
        return value.map((entry) => sanitizeValue(entry, seen, depth + 1));
    }
    if (value instanceof Set) {
        return Array.from(value).map((entry) => sanitizeValue(entry, seen, depth + 1));
    }
    if (value instanceof Map) {
        const entries = Array.from(value.entries()).map(([key, entry]) => [key, entry]);
        entries.sort(([a], [b]) => String(a).localeCompare(String(b)));
        return entries.map(([key, entry]) => [String(key), sanitizeValue(entry, seen, depth + 1)]);
    }
    if (!isPlainObject(value)) {
        return String(value);
    }
    if (seen.has(value)) {
        return null;
    }
    seen.add(value);
    const keys = Object.keys(value).sort();
    const result = {};
    for (const key of keys) {
        if (key.startsWith('__')) {
            continue;
        }
        const next = sanitizeValue(value[key], seen, depth + 1);
        if (next === undefined) {
            continue;
        }
        result[key] = next;
    }
    seen.delete(value);
    return result;
}
function normalizeVec2(value) {
    if (value == null) {
        return null;
    }
    const isArrayLike = Array.isArray(value) ||
        ArrayBuffer.isView(value) ||
        (isRecord(value) && typeof value.length === 'number');
    if (!isArrayLike || Number(value.length) < 2) {
        return null;
    }
    const entries = value;
    const x = Number(entries[0]);
    const y = Number(entries[1]);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
        return null;
    }
    return [x, y];
}
const BINDING_SENTINEL = '@binding';
const SUGARCUBES_WIDGET_IGNORES = new Set([
    'cube_id',
    'default_alias',
    'instance_alias',
    'instance_id',
    'control_after_generate',
]);
function normalizeFlags(value) {
    if (value == null) {
        return null;
    }
    if (Array.isArray(value)) {
        return value.length ? value : null;
    }
    if (isPlainObject(value)) {
        return Object.keys(value).length ? value : null;
    }
    return value;
}
function normalizeExecutionMode(value) {
    if (typeof value !== 'number' || !Number.isInteger(value) || value <= 0) {
        return null;
    }
    return value;
}
function isLinkValue(value) {
    if (!Array.isArray(value) || value.length !== 2) {
        return false;
    }
    const source = value[0];
    return typeof source === 'string' || typeof source === 'number';
}
function extractWidgetEntries(inputs) {
    if (!isRecord(inputs)) {
        return [];
    }
    const entries = [];
    for (const [name, value] of Object.entries(inputs)) {
        if (SUGARCUBES_WIDGET_IGNORES.has(name)) {
            continue;
        }
        if (name === BINDING_SENTINEL) {
            continue;
        }
        if (isLinkValue(value)) {
            continue;
        }
        if (Array.isArray(value) && value.some(isLinkValue)) {
            continue;
        }
        entries.push({
            name,
            value: sanitizeValue(value, new WeakSet(), 0),
        });
    }
    return entries;
}
function resolveDefinitionAnchor(definition) {
    const layout = isRecord(definition.layout) ? definition.layout : {};
    const groups = Array.isArray(layout.groups) ? layout.groups : [];
    for (const group of groups) {
        const groupRecord = isRecord(group) ? group : {};
        const sugarcubes = isRecord(groupRecord.sugarcubes) ? groupRecord.sugarcubes : {};
        const bounds = isRecord(sugarcubes.bounds) ? sugarcubes.bounds : null;
        if (!bounds) {
            continue;
        }
        const x = Number(bounds.x);
        const y = Number(bounds.y);
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
            continue;
        }
        const padding = isRecord(bounds.padding) ? bounds.padding : {};
        const header = isRecord(bounds.header) ? bounds.header : {};
        const headerHeight = Number(header.height) || 0;
        const padX = Number(padding.x) || 0;
        const padY = Number(padding.y) || 0;
        const topExtra = Number(padding.top_extra) || 0;
        return [x + padX, y + padY + topExtra + headerHeight];
    }
    return null;
}
/**
 * Build definition snapshot.
 */
export function buildDefinitionSnapshot(definition) {
    if (!isRecord(definition)) {
        return null;
    }
    const definitionAnchor = resolveDefinitionAnchor(definition);
    const nodes = [];
    const nodeEntries = Array.isArray(definition.nodes) ? definition.nodes : [];
    for (const entry of nodeEntries) {
        if (!isRecord(entry)) {
            continue;
        }
        const symbol = typeof entry.symbol === 'string' ? entry.symbol : null;
        if (!symbol) {
            continue;
        }
        const layout = isRecord(entry.layout) ? entry.layout : {};
        const extras = isRecord(entry.extras) ? entry.extras : {};
        const widgets = extractWidgetEntries(entry.inputs);
        let pos = normalizeVec2(layout.pos);
        if (pos && definitionAnchor) {
            pos = [pos[0] - definitionAnchor[0], pos[1] - definitionAnchor[1]];
        }
        nodes.push({
            id: symbol,
            type: typeof entry.class_type === 'string' ? entry.class_type : '',
            title: typeof layout.title === 'string' ? layout.title : '',
            mode: normalizeExecutionMode(entry.mode ?? extras.mode),
            pos,
            size: null,
            flags: normalizeFlags(sanitizeValue(layout.flags, new WeakSet(), 0)),
            properties: sanitizeValue(extras.properties, new WeakSet(), 0),
            widgets,
        });
    }
    const markerEntries = Array.isArray(definition.markers) ? definition.markers : [];
    for (const entry of markerEntries) {
        if (!isRecord(entry)) {
            continue;
        }
        const alias = typeof entry.alias === 'string' ? entry.alias : null;
        if (!alias) {
            continue;
        }
        const layout = isRecord(entry.layout) ? entry.layout : {};
        const extras = isRecord(entry.extras) ? entry.extras : {};
        const widgets = extractWidgetEntries(entry.widget_values);
        let pos = normalizeVec2(layout.pos);
        if (pos && definitionAnchor) {
            pos = [pos[0] - definitionAnchor[0], pos[1] - definitionAnchor[1]];
        }
        nodes.push({
            id: alias,
            type: typeof entry.class_type === 'string' ? entry.class_type : '',
            title: typeof layout.title === 'string' ? layout.title : '',
            mode: normalizeExecutionMode(entry.mode ?? extras.mode),
            pos,
            size: null,
            flags: normalizeFlags(sanitizeValue(layout.flags, new WeakSet(), 0)),
            properties: null,
            widgets,
        });
    }
    nodes.sort((a, b) => String(a.id).localeCompare(String(b.id)));
    const links = [];
    const connections = Array.isArray(definition.connections) ? definition.connections : [];
    for (const connection of connections) {
        const connectionRecord = isRecord(connection) ? connection : {};
        const from = isRecord(connectionRecord.from) ? connectionRecord.from : {};
        const to = isRecord(connectionRecord.to) ? connectionRecord.to : {};
        const origin = from.symbol;
        const target = to.symbol;
        const targetInput = to.input;
        if (!origin || !target || !targetInput) {
            continue;
        }
        links.push({
            origin: String(origin),
            origin_slot: from.slot ?? 0,
            target: String(target),
            target_slot: String(targetInput),
            type: '',
        });
    }
    links.sort((a, b) => {
        const keyA = `${a.origin}:${a.origin_slot}->${a.target}:${a.target_slot}`;
        const keyB = `${b.origin}:${b.origin_slot}->${b.target}:${b.target_slot}`;
        return keyA.localeCompare(keyB);
    });
    const payload = { nodes, links, group: null };
    return payload;
}
function stripSugarcubesProperties(value) {
    if (!isPlainObject(value)) {
        return value;
    }
    const volatileKeys = new Set(['cnr_id', 'ver', 'Node name for S&R', 'aux_id', 'proxyWidgets']);
    const result = {};
    for (const [key, entry] of Object.entries(value)) {
        if (key.startsWith('sugarcubes_')) {
            continue;
        }
        if (volatileKeys.has(key)) {
            continue;
        }
        result[key] = entry;
    }
    return Object.keys(result).length ? result : null;
}
function snapshotNode(node, anchor, snapshotId, options = {}) {
    if (!node) {
        return null;
    }
    let pos = normalizeVec2(node.pos);
    if (pos && anchor) {
        pos = [pos[0] - anchor[0], pos[1] - anchor[1]];
    }
    const widgets = Array.isArray(node.widgets)
        ? node.widgets
            .map((widget) => {
            if (!widget || typeof widget.name !== 'string') {
                return null;
            }
            if (SUGARCUBES_WIDGET_IGNORES.has(widget.name)) {
                return null;
            }
            return {
                name: widget.name,
                value: sanitizeValue(widget.value, new WeakSet(), 0),
            };
        })
            .filter((widget) => widget !== null)
        : [];
    const rawProperties = options.stripSugarcubesProperties
        ? stripSugarcubesProperties(node.properties)
        : node.properties;
    return {
        id: String(snapshotId || node.id),
        type: node.type || '',
        title: typeof node.title === 'string' ? node.title : '',
        mode: normalizeExecutionMode(node.mode),
        pos,
        size: null,
        flags: normalizeFlags(sanitizeValue(node.flags, new WeakSet(), 0)),
        properties: sanitizeValue(rawProperties, new WeakSet(), 0),
        widgets,
    };
}
function snapshotLinks(graph, idSet, idLookup = null, useInputNames = false, nodeById = null) {
    const links = collectGraphLinks(graph);
    const entries = [];
    for (const link of links) {
        const originId = link.origin_id ?? link.origin;
        const targetId = link.target_id ?? link.target;
        if (originId == null || targetId == null) {
            continue;
        }
        const originKey = String(originId);
        const targetKey = String(targetId);
        if (!idSet.has(originKey) || !idSet.has(targetKey)) {
            continue;
        }
        const targetSlot = link.target_slot ?? 0;
        let targetValue = targetSlot;
        if (useInputNames) {
            const node = nodeById?.get(targetKey);
            const input = Array.isArray(node?.inputs) ? node.inputs[targetSlot] : null;
            const name = typeof input?.name === 'string' ? input.name : null;
            targetValue = name || String(targetSlot);
        }
        entries.push({
            origin: idLookup?.get(originKey) || originKey,
            origin_slot: link.origin_slot ?? 0,
            target: idLookup?.get(targetKey) || targetKey,
            target_slot: targetValue,
            type: '',
        });
    }
    entries.sort((a, b) => {
        const keyA = `${a.origin}:${a.origin_slot}->${a.target}:${a.target_slot}`;
        const keyB = `${b.origin}:${b.origin_slot}->${b.target}:${b.target_slot}`;
        return keyA.localeCompare(keyB);
    });
    return entries;
}
/**
 * Snapshot group.
 */
export function snapshotGroup(group, includePosition) {
    if (!group) {
        return null;
    }
    return {
        title: typeof group.title === 'string' ? group.title : '',
        pos: includePosition ? normalizeVec2(group.pos) : null,
        size: includePosition ? normalizeVec2(group.size) : null,
        color: sanitizeValue(group.color, new WeakSet(), 0),
        bgcolor: sanitizeValue(group.bgcolor, new WeakSet(), 0),
        font_size: sanitizeValue(group.font_size, new WeakSet(), 0),
        flags: normalizeFlags(sanitizeValue(group.flags, new WeakSet(), 0)),
    };
}
/**
 * Resolve instance anchor from nodes.
 */
export function resolveInstanceAnchorFromNodes(nodeById, ids) {
    let minX = Infinity;
    let minY = Infinity;
    for (const id of ids) {
        const node = nodeById.get(id);
        const bounds = readNodeBounds(node);
        if (!bounds) {
            continue;
        }
        const [x, y] = bounds;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
    }
    if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
        return null;
    }
    return [minX, minY];
}
/**
 * Build id lookup.
 */
export function buildIdLookup(nodeById, ids) {
    const lookup = new Map();
    let missing = false;
    for (const id of ids) {
        const node = nodeById.get(id);
        const symbol = node?.properties?.sugarcubes_symbol;
        if (typeof symbol !== 'string' || !symbol.trim()) {
            missing = true;
            continue;
        }
        lookup.set(id, symbol.trim());
    }
    return { lookup, missing };
}
/**
 * Snapshot instance.
 */
export function snapshotInstance(graph, nodeIds, markerIds, anchor, groupSnapshot, options = {}) {
    const ids = new Set();
    for (const id of nodeIds || []) {
        ids.add(String(id));
    }
    for (const id of markerIds || []) {
        ids.add(String(id));
    }
    const nodeById = new Map(getGraphNodes(graph)
        .filter((node) => node?.id != null)
        .map((node) => [String(node.id), node]));
    const nodes = [];
    const idLookup = options.idLookup instanceof Map ? options.idLookup : null;
    const symbolLookup = options.useSymbols && idLookup ? idLookup : null;
    const useSymbols = symbolLookup !== null;
    const orderedIds = Array.from(ids);
    if (useSymbols) {
        orderedIds.sort((a, b) => {
            const left = symbolLookup?.get(a) || a;
            const right = symbolLookup?.get(b) || b;
            return String(left).localeCompare(String(right));
        });
    }
    else {
        orderedIds.sort((a, b) => String(a).localeCompare(String(b)));
    }
    for (const id of orderedIds) {
        const node = nodeById.get(id);
        const snapshotId = symbolLookup?.get(id) || id;
        const snapshot = snapshotNode(node, anchor, snapshotId, {
            stripSugarcubesProperties: options.stripSugarcubesProperties,
        });
        if (snapshot) {
            nodes.push(snapshot);
        }
    }
    const links = snapshotLinks(graph, ids, symbolLookup, options.useInputNames, nodeById);
    const payload = { nodes, links, group: groupSnapshot || null };
    return payload;
}
