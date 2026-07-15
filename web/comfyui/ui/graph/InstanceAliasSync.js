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
 * Own SugarCubes graph-local instance alias synchronization.
 */
import { getGraphGroups } from './GraphQuery.js';
import { updateMarkersForCubeId, updateMarkersForIds } from './CubeMarkers.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
import { isRecord } from '../types/common.js';
const groupState = new WeakMap();
function readName(value) {
    return typeof value === 'string' ? value.trim() : '';
}
function normalizeInstanceAlias(value) {
    return readName(value).toLowerCase();
}
function ensureState(group) {
    if (!group) {
        return null;
    }
    let state = groupState.get(group);
    if (!state) {
        state = { current: readName(group.title), mute: false, onChange: null };
        groupState.set(group, state);
    }
    return state;
}
/**
 * Ensure group title watcher.
 */
export function ensureGroupTitleWatcher(group, onChange) {
    if (!group || group.__sugarcubes_title_watch) {
        return;
    }
    const state = ensureState(group);
    if (!state) {
        return;
    }
    state.current = readName(group.title);
    state.onChange = onChange;
    Object.defineProperty(group, 'title', {
        get() {
            return state.current;
        },
        set(value) {
            const next = readName(value);
            const prev = state.current;
            state.current = next;
            if (!state.mute && prev !== next) {
                state.onChange?.(group, next, prev);
            }
        },
        enumerable: true,
        configurable: true,
    });
    group.__sugarcubes_title_watch = true;
}
/**
 * Set group title.
 */
export function setGroupTitle(group, value) {
    if (!group) {
        return;
    }
    const state = groupState.get(group);
    if (state) {
        state.mute = true;
        group.title = value;
        state.mute = false;
        return;
    }
    group.title = value;
}
/**
 * Allocate unique graph-local instance alias.
 */
export function allocateUniqueInstanceAlias(graph, desiredName, options = {}) {
    const base = readName(desiredName) || 'SugarCube';
    if (!graph) {
        return base;
    }
    const currentInstanceId = typeof options.currentInstanceId === 'string' ? options.currentInstanceId.trim() : '';
    const currentGroup = options.currentGroup ?? null;
    const ignoreGroup = typeof options.ignoreGroup === 'function' ? options.ignoreGroup : null;
    const taken = new Set();
    const groups = getGraphGroups(graph);
    for (const group of groups) {
        if (!group || (currentGroup && group === currentGroup)) {
            continue;
        }
        const metadata = getGroupSugarcubes(group);
        if (!metadata?.managed) {
            continue;
        }
        if (ignoreGroup?.(group, metadata)) {
            continue;
        }
        const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
        if (currentInstanceId && instanceId === currentInstanceId) {
            continue;
        }
        const name = normalizeInstanceAlias(metadata.instance_alias);
        if (name) {
            taken.add(name);
        }
    }
    const baseKey = normalizeInstanceAlias(base);
    if (!baseKey || !taken.has(baseKey)) {
        return base;
    }
    let index = 2;
    while (index < 1000) {
        const next = `${base} ${index}`;
        if (!taken.has(normalizeInstanceAlias(next))) {
            return next;
        }
        index += 1;
    }
    return `${base} ${Date.now()}`;
}
/**
 * Find group for cube id.
 */
export function findGroupForCubeId(graph, cubeId) {
    if (!graph || !cubeId) {
        return null;
    }
    const groups = getGraphGroups(graph);
    for (const group of groups) {
        const metadata = getGroupSugarcubes(group);
        const groupCubeId = readName(metadata?.cube_id);
        if (groupCubeId && groupCubeId === cubeId) {
            return group;
        }
    }
    return null;
}
/**
 * Find group for marker id.
 */
export function findGroupForMarkerId(graph, markerId) {
    if (!graph || markerId == null) {
        return null;
    }
    const markerKey = String(markerId);
    const groups = getGraphGroups(graph);
    for (const group of groups) {
        const metadata = getGroupSugarcubes(group);
        const markers = metadata?.markers;
        if (!isRecord(markers)) {
            continue;
        }
        const ids = [
            ...(Array.isArray(markers.inputs) ? markers.inputs : []),
            ...(Array.isArray(markers.outputs) ? markers.outputs : []),
        ].map((value) => String(value));
        if (ids.includes(markerKey)) {
            return group;
        }
    }
    return null;
}
/**
 * Find group for node id.
 */
export function findGroupForNodeId(graph, nodeId) {
    if (!graph || nodeId == null) {
        return null;
    }
    const nodeKey = String(nodeId);
    const groups = getGraphGroups(graph);
    for (const group of groups) {
        const metadata = getGroupSugarcubes(group);
        if (!metadata) {
            continue;
        }
        const nodeIds = Array.isArray(metadata.nodes) ? metadata.nodes : [];
        if (nodeIds.map((value) => String(value)).includes(nodeKey)) {
            return group;
        }
        const markers = metadata.markers;
        if (!isRecord(markers)) {
            continue;
        }
        const markerIds = [
            ...(Array.isArray(markers.inputs) ? markers.inputs : []),
            ...(Array.isArray(markers.outputs) ? markers.outputs : []),
        ].map((value) => String(value));
        if (markerIds.includes(nodeKey)) {
            return group;
        }
    }
    return null;
}
/**
 * Sync graph-local instance alias.
 */
export function syncInstanceAlias({ graph, group, metadata, cubeId, instanceAlias, events, requestDirtyRefresh, } = {}) {
    const next = readName(instanceAlias);
    if (!next) {
        return { instanceAlias: '', updated: false };
    }
    let updated = false;
    if (metadata && readName(metadata.instance_alias) !== next) {
        metadata.instance_alias = next;
        updated = true;
    }
    if (group && readName(group.title) !== next) {
        setGroupTitle(group, next);
        updated = true;
    }
    if (graph && isRecord(metadata?.markers)) {
        const markerIds = [
            ...(Array.isArray(metadata.markers.inputs) ? metadata.markers.inputs : []),
            ...(Array.isArray(metadata.markers.outputs) ? metadata.markers.outputs : []),
        ];
        const updatedMarkers = updateMarkersForIds(graph, markerIds, { instanceAlias: next });
        if (updatedMarkers) {
            updated = true;
        }
    }
    else if (graph && cubeId) {
        const updatedMarkers = updateMarkersForCubeId(graph, cubeId, { instanceAlias: next });
        if (updatedMarkers) {
            updated = true;
        }
    }
    if (updated && events?.emit) {
        events.emit('cube:instances:refresh', { graph, reason: 'cube-instance-alias-sync' });
    }
    if (updated && requestDirtyRefresh) {
        requestDirtyRefresh({
            ...(graph === undefined ? {} : { graph }),
            reason: 'cube-instance-alias-sync',
        });
    }
    return { instanceAlias: next, updated };
}
