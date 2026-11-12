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
 * Own reusable group discovery and legacy marker-group reconciliation.
 */
import { getNodeCenter, isPointInBounds, readGroupBounds } from './Bounds.js';
import { buildMarkerSignature, readMarkerIdsFromMetadata } from '../layout/CubeInstanceIndex.js';
import { getGroupSugarcubes } from './GroupMetadata.js';
/** Match discovered instances to authoritative or safely reusable graph groups. */
export class InstanceGroupReconciler {
    resolve(instances, groups) {
        const byInstance = new Map();
        const byMarkers = new Map();
        for (const group of groups) {
            const data = getGroupSugarcubes(group);
            const instanceId = readMetadataString(data?.instance_id);
            if (instanceId)
                byInstance.set(instanceId, group);
            if (data?.managed) {
                const signature = buildMarkerSignature(readMarkerIdsFromMetadata(data));
                if (signature)
                    byMarkers.set(signature, group);
            }
        }
        const claimed = new Set();
        const matches = instances.map((instance, order) => {
            const markerSignature = buildMarkerSignature(instance.markerIds);
            let group = byInstance.get(instance.instanceId) ||
                (markerSignature ? byMarkers.get(markerSignature) : null) ||
                null;
            if (group && getGroupSugarcubes(group) && claimed.has(group))
                group = null;
            if (!group)
                group = this.findReusableGroup(instance, groups.filter((entry) => !claimed.has(entry)));
            if (group && getGroupSugarcubes(group))
                claimed.add(group);
            return { instance, group, order };
        });
        return this.mergeMarkerOnlyMatches(matches);
    }
    buildSignature(instances, groups) {
        const instanceSignature = instances
            .map((instance) => {
            const ids = instance.nodeIds.concat(instance.markerIds).sort();
            return `${instance.instanceId}:${instance.cubeDefinitionKey || instance.cubeId}:${ids.join(',')}`;
        })
            .sort()
            .join('|');
        const titleSignature = groups
            .map((group) => {
            const data = getGroupSugarcubes(group);
            if (!data?.managed || !data.instance_id)
                return '';
            return `${data.instance_id}:${typeof group.title === 'string' ? group.title.trim() : ''}`;
        })
            .filter(Boolean)
            .sort()
            .join('|');
        return `${instanceSignature}::${titleSignature}`;
    }
    findReusableGroup(instance, groups) {
        let best = null;
        let bestArea = null;
        for (const group of groups) {
            if (!group || getGroupSugarcubes(group))
                continue;
            const bounds = readGroupBounds(group);
            if (!bounds)
                continue;
            const allInside = [...instance.nodes, ...instance.markers].every((node) => {
                const center = getNodeCenter(node);
                return center ? isPointInBounds(center, bounds) : false;
            });
            if (!allInside)
                continue;
            const area = bounds[2] * bounds[3];
            if (bestArea == null || area < bestArea) {
                best = group;
                bestArea = area;
            }
        }
        return best;
    }
    mergeMarkerOnlyMatches(matches) {
        const grouped = new Map();
        const passthrough = [];
        for (const match of matches) {
            if (!match.group || getGroupSugarcubes(match.group) || match.instance.nodeIds.length > 0) {
                passthrough.push(match);
                continue;
            }
            grouped.set(match.group, [...(grouped.get(match.group) || []), match]);
        }
        const merged = [...passthrough];
        for (const groupMatches of grouped.values()) {
            if (groupMatches.length === 1) {
                const match = groupMatches[0];
                if (match)
                    merged.push(match);
                continue;
            }
            if (new Set(groupMatches.map((match) => match.instance.cubeDefinitionKey || '')).size > 1) {
                merged.push(...groupMatches);
                continue;
            }
            const ordered = [...groupMatches].sort((left, right) => left.order - right.order);
            const first = ordered[0];
            if (!first)
                continue;
            const canonical = {
                ...first.instance,
                instanceId: groupMatches
                    .map((match) => match.instance.instanceId)
                    .filter(Boolean)
                    .sort()[0] || first.instance.instanceId,
                markerLookup: { inputs: [], outputs: [] },
                nodeIds: [],
                markerIds: [],
                nodes: [],
                markers: [],
            };
            for (const match of ordered) {
                canonical.markerLookup.inputs = mergeLists(canonical.markerLookup.inputs, match.instance.markerLookup?.inputs);
                canonical.markerLookup.outputs = mergeLists(canonical.markerLookup.outputs, match.instance.markerLookup?.outputs);
                canonical.markerIds = mergeLists(canonical.markerIds, match.instance.markerIds);
                canonical.markers = mergeLists(canonical.markers, match.instance.markers);
            }
            merged.push({ ...first, instance: canonical });
        }
        return merged.sort((left, right) => left.order - right.order);
    }
}
function mergeLists(left = [], right = []) {
    return Array.from(new Set([...left, ...right]));
}
function readMetadataString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
