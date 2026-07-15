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
 * Own the SugarCubes layout orchestration layer in `web/comfyui/ui/layout/CubeInstanceIndex.js`.
 */
import { resolveInstanceBounds } from '../graph/CubeBounds.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { InstanceBuilder } from '../graph/InstanceBuilder.js';
import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
/**
 * Read marker ids from metadata.
 */
export function readMarkerIdsFromMetadata(metadata) {
    if (!isRecord(metadata?.markers)) {
        return [];
    }
    const markers = metadata.markers;
    return [
        ...(Array.isArray(markers.inputs) ? markers.inputs : []),
        ...(Array.isArray(markers.outputs) ? markers.outputs : []),
    ];
}
/**
 * Build marker signature.
 */
export function buildMarkerSignature(markerIds) {
    if (!Array.isArray(markerIds) || !markerIds.length) {
        return '';
    }
    return markerIds
        .map((value) => String(value))
        .sort()
        .join('|');
}
function buildGroupIndex(groups) {
    const byInstanceId = new Map();
    const byMarkerSignature = new Map();
    for (const group of groups) {
        const metadata = getGroupSugarcubes(group);
        if (metadata?.instance_id) {
            byInstanceId.set(String(metadata.instance_id), { group, metadata });
        }
        if (metadata?.managed) {
            const signatureKey = buildMarkerSignature(readMarkerIdsFromMetadata(metadata));
            if (signatureKey) {
                byMarkerSignature.set(signatureKey, { group, metadata });
            }
        }
    }
    return { byInstanceId, byMarkerSignature };
}
function attachGroup(instance, groupsIndex) {
    const direct = groupsIndex.byInstanceId.get(instance.instanceId) || null;
    if (direct) {
        return direct;
    }
    const markerSignature = buildMarkerSignature(instance.markerIds);
    if (!markerSignature) {
        return null;
    }
    return groupsIndex.byMarkerSignature.get(markerSignature) || null;
}
function buildIndex({ graph, instanceBuilder, groups, }) {
    const resolvedGroups = Array.isArray(groups) ? groups : getGraphGroups(graph);
    const builder = instanceBuilder || new InstanceBuilder({ logger: null });
    const groupIndex = buildGroupIndex(resolvedGroups);
    const instances = builder.build(graph);
    const instanceById = new Map();
    const instanceByCubeId = new Map();
    const instanceByMarkerId = new Map();
    const instanceByNodeId = new Map();
    const enriched = instances.map((instance) => {
        const matched = attachGroup(instance, groupIndex);
        const group = matched?.group ?? null;
        const metadata = matched?.metadata ?? null;
        const bounds = resolveInstanceBounds({
            group,
            metadata,
            nodes: instance.nodes,
            markers: instance.markers,
        });
        const entry = { ...instance, group, metadata, bounds };
        if (entry.instanceId) {
            instanceById.set(entry.instanceId, entry);
        }
        if (entry.cubeId) {
            instanceByCubeId.set(entry.cubeId, entry);
        }
        for (const node of entry.nodes || []) {
            const nodeId = node?.id != null ? String(node.id) : '';
            if (nodeId && !instanceByNodeId.has(nodeId)) {
                instanceByNodeId.set(nodeId, entry);
            }
        }
        for (const markerId of entry.markerIds || []) {
            const markerKey = String(markerId);
            if (!instanceByMarkerId.has(markerKey)) {
                instanceByMarkerId.set(markerKey, entry);
            }
        }
        return entry;
    });
    return {
        instances: enriched,
        instanceById,
        instanceByCubeId,
        instanceByMarkerId,
        instanceByNodeId,
    };
}
/**
 * Coordinate cube instance index behavior for the SugarCubes UI.
 */
export class CubeInstanceIndex {
    graph;
    instanceBuilder;
    groups;
    instances;
    instanceById;
    instanceByCubeId;
    instanceByMarkerId;
    instanceByNodeId;
    constructor({ graph, instanceBuilder, groups, } = {}) {
        this.graph = graph || null;
        this.instanceBuilder = instanceBuilder || null;
        this.groups = Array.isArray(groups) ? groups : null;
        const { instances, instanceById, instanceByCubeId, instanceByMarkerId, instanceByNodeId } = buildIndex({
            graph: this.graph,
            instanceBuilder: this.instanceBuilder,
            groups: this.groups,
        });
        this.instances = instances;
        this.instanceById = instanceById;
        this.instanceByCubeId = instanceByCubeId;
        this.instanceByMarkerId = instanceByMarkerId;
        this.instanceByNodeId = instanceByNodeId;
    }
}
