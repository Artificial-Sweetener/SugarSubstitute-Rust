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
 * Own the SugarCubes host integration layer in `web/js/cube_creation.js`.
 */
import { app } from '/scripts/app.js';
import { buildLinkIndex, getGraphNodes } from '../comfyui/ui/graph/GraphQuery.js';
import { readWidgetValue, writeWidgetValue } from '../comfyui/ui/graph/Markers.js';
import { getSugarCubesUI } from '../comfyui/ui/index.js';
const MARKER_CLASS_TYPES = new Set(['SugarCubes.CubeInput', 'SugarCubes.CubeOutput']);
const MARKER_KIND_BY_TYPE = {
    'SugarCubes.CubeInput': 'input',
    'SugarCubes.CubeOutput': 'output',
};
const MARKER_TYPE_BY_KIND = {
    input: 'SugarCubes.CubeInput',
    output: 'SugarCubes.CubeOutput',
};
const MARKER_OFFSET = { x: 24, y: 28 };
const DEFAULT_MARKER_SIZE = { w: 140, h: 46 };
const LITEGRAPH_REF = typeof LiteGraph !== 'undefined' ? LiteGraph : null;
const ui = getSugarCubesUI();
function isMarker(node) {
    return typeof node?.type === 'string' && MARKER_CLASS_TYPES.has(node.type);
}
function coerceVec2(value, fallback = [0, 0]) {
    if ((!Array.isArray(value) && !ArrayBuffer.isView(value)) ||
        Number(value.length) < 2) {
        return fallback;
    }
    const sequence = value;
    const x = Number(sequence[0]);
    const y = Number(sequence[1]);
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
        return fallback;
    }
    return [x, y];
}
function resolveOutputSlotIndex(node, slot) {
    if (typeof slot === 'number' && Number.isFinite(slot)) {
        return slot;
    }
    if (typeof slot === 'string' && Array.isArray(node?.outputs)) {
        const idx = node.outputs.findIndex((output) => output?.name === slot);
        if (idx >= 0) {
            return idx;
        }
    }
    return 0;
}
function resolveInputSlotIndex(node, slot) {
    if (typeof slot === 'number' && Number.isFinite(slot)) {
        return slot;
    }
    if (typeof slot === 'string' && Array.isArray(node?.inputs)) {
        const idx = node.inputs.findIndex((input) => input?.name === slot);
        if (idx >= 0) {
            return idx;
        }
    }
    return 0;
}
function getNodeById(graph, nodeId) {
    if (!graph || nodeId == null)
        return null;
    if (typeof graph.getNodeById === 'function') {
        return graph.getNodeById(nodeId) ?? null;
    }
    return (getGraphNodes(graph).find((node) => node?.id === nodeId) || null);
}
function removeLink(graph, link) {
    if (!graph || !link)
        return;
    if (typeof graph.removeLink === 'function' && link.id != null) {
        graph.removeLink(link.id);
        return;
    }
    const originId = link.origin_id ?? link.origin;
    const targetId = link.target_id ?? link.target;
    const originNode = getNodeById(graph, originId);
    const targetNode = getNodeById(graph, targetId);
    if (originNode?.disconnectOutput) {
        try {
            originNode.disconnectOutput(link.origin_slot ?? 0, targetNode);
            return;
        }
        catch (_error) {
            // fall through
        }
    }
    if (targetNode?.disconnectInput) {
        try {
            targetNode.disconnectInput(link.target_slot ?? 0);
        }
        catch (_error) {
            // ignore disconnect failures
        }
    }
}
function connectNodes(fromNode, fromSlot, toNode, toSlot) {
    if (!fromNode?.connect || !toNode)
        return false;
    const outputIndex = resolveOutputSlotIndex(fromNode, fromSlot);
    const inputIndex = resolveInputSlotIndex(toNode, toSlot);
    try {
        fromNode.connect(outputIndex, toNode, inputIndex);
        return true;
    }
    catch (_error) {
        return false;
    }
}
function markerPositionFor(kind, anchorNode, index, total) {
    const pos = coerceVec2(anchorNode?.pos, null);
    if (!pos) {
        return [0, 0];
    }
    const size = coerceVec2(anchorNode?.size, [180, 60]) ?? [180, 60];
    const count = Number.isFinite(total) && total > 0 ? total : 1;
    const baseY = pos[1] + size[1] / 2 - DEFAULT_MARKER_SIZE.h / 2;
    const stride = DEFAULT_MARKER_SIZE.h + MARKER_OFFSET.y;
    const offsetIndex = index - (count - 1) / 2;
    const offsetY = offsetIndex * stride;
    if (kind === 'output') {
        return [pos[0] + size[0] + MARKER_OFFSET.x, baseY + offsetY];
    }
    return [pos[0] - MARKER_OFFSET.x, baseY + offsetY];
}
function createMarkerNode(graph, kind, defaultAlias, cubeId, pos) {
    const classType = MARKER_TYPE_BY_KIND[kind] || MARKER_TYPE_BY_KIND.input;
    const node = LITEGRAPH_REF?.createNode
        ? LITEGRAPH_REF.createNode(classType)
        : null;
    if (!node) {
        return null;
    }
    if (Array.isArray(pos)) {
        node.pos = [pos[0], pos[1]];
    }
    graph.add(node);
    ensureSymbol(node, 'marker', node.id);
    if (cubeId) {
        writeWidgetValue(node, 'cube_id', cubeId);
    }
    writeWidgetValue(node, 'default_alias', defaultAlias);
    writeWidgetValue(node, 'instance_alias', defaultAlias);
    return node;
}
function ensureSymbol(node, prefix, fallback) {
    if (!node) {
        return;
    }
    if (!node.properties) {
        node.properties = {};
    }
    if (typeof node.properties.sugarcubes_symbol === 'string') {
        return;
    }
    const id = node.id ?? fallback ?? Math.floor(Math.random() * 1e6);
    node.properties.sugarcubes_symbol = `${prefix}_${id}`;
}
function ensureSymbolsForNodes(nodes, prefix) {
    nodes.forEach((node, index) => {
        ensureSymbol(node, prefix, index);
    });
}
function scheduleInstanceRefresh(graph, reason) {
    ui?.scheduleCubeInstanceRefresh?.({ graph, reason });
}
function buildBoundaryLinks(graph, selection) {
    const selectedIds = new Set(selection.map((node) => String(node.id)));
    const { links } = buildLinkIndex(graph);
    const incoming = [];
    const outgoing = [];
    for (const link of links) {
        const originId = link.origin_id ?? link.origin;
        const targetId = link.target_id ?? link.target;
        if (originId == null || targetId == null) {
            continue;
        }
        const originKey = String(originId);
        const targetKey = String(targetId);
        const originNode = getNodeById(graph, originId);
        const targetNode = getNodeById(graph, targetId);
        if (!originNode || !targetNode) {
            continue;
        }
        if (isMarker(originNode) || isMarker(targetNode)) {
            continue;
        }
        const originIn = selectedIds.has(originKey);
        const targetIn = selectedIds.has(targetKey);
        if (originIn && !targetIn) {
            outgoing.push({ link, originNode, targetNode });
        }
        else if (!originIn && targetIn) {
            incoming.push({ link, originNode, targetNode });
        }
    }
    return { incoming, outgoing };
}
function reachableFromMarker(graph, markerNode) {
    const kind = (markerNode.type ? MARKER_KIND_BY_TYPE[markerNode.type] : undefined) || 'input';
    const { outgoing, incoming } = buildLinkIndex(graph);
    const visited = new Set();
    const queue = [];
    const startId = String(markerNode.id);
    if (kind === 'input') {
        const edges = outgoing.get(startId) ?? [];
        for (const edge of edges) {
            const targetId = edge.target_id ?? edge.target;
            if (targetId == null)
                continue;
            const targetKey = String(targetId);
            if (visited.has(targetKey))
                continue;
            visited.add(targetKey);
            queue.push(targetKey);
        }
    }
    else if (kind === 'output') {
        const edges = incoming.get(startId) ?? [];
        for (const edge of edges) {
            const sourceId = edge.origin_id ?? edge.origin;
            if (sourceId == null)
                continue;
            const sourceKey = String(sourceId);
            if (visited.has(sourceKey))
                continue;
            visited.add(sourceKey);
            queue.push(sourceKey);
        }
    }
    else {
        visited.add(startId);
        queue.push(startId);
    }
    while (queue.length) {
        const current = queue.shift();
        if (current === undefined) {
            break;
        }
        if (kind !== 'output') {
            const outEdges = outgoing.get(current) ?? [];
            for (const edge of outEdges) {
                const targetId = edge.target_id ?? edge.target;
                if (targetId == null)
                    continue;
                const targetKey = String(targetId);
                if (visited.has(targetKey))
                    continue;
                visited.add(targetKey);
                queue.push(targetKey);
            }
        }
        if (kind === 'input') {
            continue;
        }
        const inEdges = incoming.get(current) ?? [];
        for (const edge of inEdges) {
            const sourceId = edge.origin_id ?? edge.origin;
            if (sourceId == null)
                continue;
            const sourceKey = String(sourceId);
            if (visited.has(sourceKey))
                continue;
            visited.add(sourceKey);
            queue.push(sourceKey);
        }
    }
    visited.delete(startId);
    return visited;
}
function validateMarkerDefaultAliases(markers, expectedDefaultAlias) {
    const names = new Set();
    for (const marker of markers) {
        const value = readWidgetValue(marker, 'default_alias');
        if (typeof value === 'string' && value.trim()) {
            names.add(value.trim());
        }
    }
    if (expectedDefaultAlias) {
        names.add(expectedDefaultAlias);
    }
    if (names.size > 1) {
        return { ok: false, message: 'Multiple default aliases detected in the selection.' };
    }
    return { ok: true, message: '' };
}
function validateMarkerIds(markers, expectedId) {
    const ids = new Set();
    for (const marker of markers) {
        const value = readWidgetValue(marker, 'cube_id');
        if (typeof value === 'string' && value.trim()) {
            ids.add(value.trim());
        }
    }
    if (expectedId) {
        ids.add(expectedId);
    }
    if (ids.size > 1) {
        return { ok: false, message: 'Multiple cube ids detected in the selection.' };
    }
    return { ok: true, message: '' };
}
/**
 * Create cube from selection.
 */
export function createCubeFromSelection({ graph, defaultAlias, selection, }) {
    if (!graph) {
        return { ok: false, message: 'Graph unavailable.' };
    }
    const trimmed = typeof defaultAlias === 'string' ? defaultAlias.trim() : '';
    if (!trimmed) {
        return { ok: false, message: 'Default alias is required.' };
    }
    const nodes = Array.isArray(selection) ? selection.filter((node) => !isMarker(node)) : [];
    if (!nodes.length) {
        return { ok: false, message: 'Select at least one non-marker node.' };
    }
    ensureSymbolsForNodes(nodes, 'node');
    const { incoming, outgoing } = buildBoundaryLinks(graph, nodes);
    if (!incoming.length && !outgoing.length) {
        return {
            ok: false,
            message: 'No boundary connections found. Select a subgraph with external links.',
        };
    }
    const cubeId = '';
    const created = [];
    const incomingGroups = new Map();
    for (const entry of incoming) {
        const key = String(entry.targetNode?.id);
        const list = incomingGroups.get(key) ?? [];
        list.push(entry);
        incomingGroups.set(key, list);
    }
    const outgoingGroups = new Map();
    for (const entry of outgoing) {
        const key = String(entry.originNode?.id);
        const list = outgoingGroups.get(key) ?? [];
        list.push(entry);
        outgoingGroups.set(key, list);
    }
    incomingGroups.forEach((entries) => {
        entries.forEach((entry, idx) => {
            removeLink(graph, entry.link);
            const pos = markerPositionFor('input', entry.targetNode, idx, entries.length);
            const marker = createMarkerNode(graph, 'input', trimmed, cubeId, pos);
            if (!marker) {
                return;
            }
            connectNodes(entry.originNode, entry.link.origin_slot, marker, 0);
            connectNodes(marker, 0, entry.targetNode, entry.link.target_slot);
            created.push(marker);
        });
    });
    outgoingGroups.forEach((entries) => {
        entries.forEach((entry, idx) => {
            removeLink(graph, entry.link);
            const pos = markerPositionFor('output', entry.originNode, idx, entries.length);
            const marker = createMarkerNode(graph, 'output', trimmed, cubeId, pos);
            if (!marker) {
                return;
            }
            connectNodes(entry.originNode, entry.link.origin_slot, marker, 0);
            connectNodes(marker, 0, entry.targetNode, entry.link.target_slot);
            created.push(marker);
        });
    });
    scheduleInstanceRefresh(graph, 'cube-create-selection');
    return {
        ok: true,
        message: `Created ${created.length} markers for ${trimmed}.`,
        markers: created,
    };
}
/**
 * Wrap marker to cube.
 */
export function wrapMarkerToCube(markerNode, options = {}) {
    const graph = (markerNode?.graph ?? app.graph);
    if (!graph || !markerNode) {
        return { ok: false, message: 'Marker unavailable.' };
    }
    const storedDefaultAlias = readWidgetValue(markerNode, 'default_alias');
    const name = typeof options.defaultAlias === 'string'
        ? options.defaultAlias.trim()
        : typeof storedDefaultAlias === 'string'
            ? storedDefaultAlias.trim()
            : '';
    if (!name) {
        return { ok: false, message: 'Default alias is required.' };
    }
    const storedCubeId = readWidgetValue(markerNode, 'cube_id');
    const markerCubeId = typeof storedCubeId === 'string' ? storedCubeId.trim() : '';
    const cubeId = markerCubeId;
    const nodes = getGraphNodes(graph);
    const reachable = reachableFromMarker(graph, markerNode);
    const selection = nodes.filter((node) => reachable.has(String(node.id)) && !isMarker(node));
    if (!selection.length) {
        return { ok: false, message: 'No nodes connected to this marker.' };
    }
    ensureSymbol(markerNode, 'marker', markerNode?.id);
    ensureSymbolsForNodes(selection, 'node');
    const componentMarkers = nodes.filter((node) => isMarker(node) && reachable.has(String(node.id)));
    const nameCheck = validateMarkerDefaultAliases(componentMarkers, name);
    if (!nameCheck.ok) {
        return nameCheck;
    }
    const idCheck = validateMarkerIds(componentMarkers, cubeId);
    if (!idCheck.ok) {
        return idCheck;
    }
    const { incoming, outgoing } = buildBoundaryLinks(graph, selection);
    if (!incoming.length && !outgoing.length) {
        return {
            ok: false,
            message: 'No boundary connections found near this marker.',
        };
    }
    const created = [];
    const incomingGroups = new Map();
    for (const entry of incoming) {
        const key = String(entry.targetNode?.id);
        const list = incomingGroups.get(key) ?? [];
        list.push(entry);
        incomingGroups.set(key, list);
    }
    const outgoingGroups = new Map();
    for (const entry of outgoing) {
        const key = String(entry.originNode?.id);
        const list = outgoingGroups.get(key) ?? [];
        list.push(entry);
        outgoingGroups.set(key, list);
    }
    incomingGroups.forEach((entries) => {
        entries.forEach((entry, idx) => {
            removeLink(graph, entry.link);
            const pos = markerPositionFor('input', entry.targetNode, idx, entries.length);
            const marker = createMarkerNode(graph, 'input', name, cubeId, pos);
            if (!marker) {
                return;
            }
            connectNodes(entry.originNode, entry.link.origin_slot, marker, 0);
            connectNodes(marker, 0, entry.targetNode, entry.link.target_slot);
            created.push(marker);
        });
    });
    outgoingGroups.forEach((entries) => {
        entries.forEach((entry, idx) => {
            removeLink(graph, entry.link);
            const pos = markerPositionFor('output', entry.originNode, idx, entries.length);
            const marker = createMarkerNode(graph, 'output', name, cubeId, pos);
            if (!marker) {
                return;
            }
            connectNodes(entry.originNode, entry.link.origin_slot, marker, 0);
            connectNodes(marker, 0, entry.targetNode, entry.link.target_slot);
            created.push(marker);
        });
    });
    scheduleInstanceRefresh(graph, 'cube-wrap-marker');
    return {
        ok: true,
        message: `Wrapped marker into ${name} with ${created.length} new markers.`,
        markers: created,
    };
}
