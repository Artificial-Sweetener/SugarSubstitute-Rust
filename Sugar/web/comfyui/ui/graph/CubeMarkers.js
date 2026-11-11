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
 * Own SugarCubes marker widget reads and writes.
 */
import { getGraphNodes } from './GraphQuery.js';
import { readWidgetValue, writeWidgetValue } from './Markers.js';
function hasOwnValue(value, key) {
    return Object.prototype.hasOwnProperty.call(value, key);
}
function normalizeUpdateString(value) {
    return typeof value === 'string' ? value : '';
}
/**
 * Expose the cube marker kinds constant.
 */
export const CUBE_MARKER_KINDS = Object.freeze({
    'SugarCubes.CubeInput': 'input',
    'SugarCubes.CubeOutput': 'output',
});
/**
 * Return whether cube marker type.
 */
export function isCubeMarkerType(node) {
    return Boolean(node?.type && CUBE_MARKER_KINDS[node.type]);
}
/**
 * Read cube marker default alias.
 */
export function readCubeMarkerDefaultAlias(node) {
    const value = readWidgetValue(node, 'default_alias');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read cube marker id.
 */
export function readCubeMarkerId(node) {
    const value = readWidgetValue(node, 'cube_id');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read cube marker instance id.
 */
export function readCubeMarkerInstanceId(node) {
    const value = readWidgetValue(node, 'instance_id');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read cube marker instance alias.
 */
export function readCubeMarkerInstanceAlias(node) {
    const value = readWidgetValue(node, 'instance_alias');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read optional cube marker version metadata.
 */
export function readCubeMarkerVersion(node) {
    const propertyValue = typeof node?.properties?.sugarcubes_cube_version === 'string'
        ? node.properties.sugarcubes_cube_version.trim()
        : '';
    if (propertyValue) {
        return propertyValue;
    }
    const value = readWidgetValue(node, 'cube_version');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Read optional cube marker revision ref metadata.
 */
export function readCubeMarkerRevisionRef(node) {
    const propertyValue = typeof node?.properties?.sugarcubes_cube_revision_ref === 'string'
        ? node.properties.sugarcubes_cube_revision_ref.trim()
        : '';
    if (propertyValue) {
        return propertyValue;
    }
    const value = readWidgetValue(node, 'cube_revision_ref');
    return typeof value === 'string' ? value.trim() : '';
}
/**
 * Write cube marker instance id.
 */
export function writeCubeMarkerInstanceId(node, instanceId) {
    if (typeof instanceId !== 'string') {
        return false;
    }
    return writeWidgetValue(node, 'instance_id', instanceId);
}
/**
 * Write cube marker instance alias.
 */
export function writeCubeMarkerInstanceAlias(node, instanceAlias) {
    if (typeof instanceAlias !== 'string') {
        return false;
    }
    return writeWidgetValue(node, 'instance_alias', instanceAlias);
}
/**
 * Update markers for cube id.
 */
export function updateMarkersForCubeId(graph, cubeId, updates) {
    if (!graph || !cubeId || !updates) {
        return 0;
    }
    const nodes = getGraphNodes(graph);
    let updated = 0;
    for (const node of nodes) {
        if (!isCubeMarkerType(node)) {
            continue;
        }
        const nodeCubeId = readCubeMarkerId(node);
        if (nodeCubeId !== cubeId) {
            continue;
        }
        if (updates.cubeId) {
            writeWidgetValue(node, 'cube_id', updates.cubeId);
        }
        if (updates.defaultAlias) {
            writeWidgetValue(node, 'default_alias', updates.defaultAlias);
        }
        if (typeof updates.instanceAlias === 'string') {
            writeWidgetValue(node, 'instance_alias', updates.instanceAlias);
        }
        if (updates.instanceId) {
            writeWidgetValue(node, 'instance_id', updates.instanceId);
        }
        if (hasOwnValue(updates, 'cubeVersion')) {
            node.properties =
                node.properties && typeof node.properties === 'object' ? node.properties : {};
            node.properties.sugarcubes_cube_version = normalizeUpdateString(updates.cubeVersion);
        }
        if (hasOwnValue(updates, 'cubeRevisionRef')) {
            node.properties =
                node.properties && typeof node.properties === 'object' ? node.properties : {};
            node.properties.sugarcubes_cube_revision_ref = normalizeUpdateString(updates.cubeRevisionRef);
        }
        updated += 1;
    }
    return updated;
}
/**
 * Update markers for ids.
 */
export function updateMarkersForIds(graph, markerIds, updates) {
    if (!graph || !updates) {
        return 0;
    }
    const ids = new Set(Array.isArray(markerIds) ? markerIds.map((value) => String(value)) : [String(markerIds)]);
    if (!ids.size) {
        return 0;
    }
    const nodes = getGraphNodes(graph);
    let updated = 0;
    for (const node of nodes) {
        if (!isCubeMarkerType(node)) {
            continue;
        }
        const nodeId = node?.id != null ? String(node.id) : '';
        if (!nodeId || !ids.has(nodeId)) {
            continue;
        }
        if (updates.cubeId) {
            writeWidgetValue(node, 'cube_id', updates.cubeId);
        }
        if (updates.defaultAlias) {
            writeWidgetValue(node, 'default_alias', updates.defaultAlias);
        }
        if (typeof updates.instanceAlias === 'string') {
            writeWidgetValue(node, 'instance_alias', updates.instanceAlias);
        }
        if (updates.instanceId) {
            writeWidgetValue(node, 'instance_id', updates.instanceId);
        }
        if (hasOwnValue(updates, 'cubeVersion')) {
            node.properties =
                node.properties && typeof node.properties === 'object' ? node.properties : {};
            node.properties.sugarcubes_cube_version = normalizeUpdateString(updates.cubeVersion);
        }
        if (hasOwnValue(updates, 'cubeRevisionRef')) {
            node.properties =
                node.properties && typeof node.properties === 'object' ? node.properties : {};
            node.properties.sugarcubes_cube_revision_ref = normalizeUpdateString(updates.cubeRevisionRef);
        }
        updated += 1;
    }
    return updated;
}
