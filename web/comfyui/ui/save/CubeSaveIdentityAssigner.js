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
/** Assign durable personal identities before Cube saves. */
import { isCanonicalCubeId } from '../core/CubeId.js';
import { ANY_TARGET_MODEL, deriveTargetModelFromCubeId, normalizeTargetModel, } from '../core/ModelTargets.js';
import { isCubeMarkerType, updateMarkersForIds } from '../graph/CubeMarkers.js';
import { InstanceBuilder } from '../graph/InstanceBuilder.js';
import { readWidgetValue } from '../graph/Markers.js';
import { suggestPersonalCubeIdentity } from '../create/PersonalCubeIdentity.js';
/** Own canonical identity assignment for unsaved Cube instances. */
export class CubeSaveIdentityAssigner {
    options;
    constructor(options) {
        this.options = options;
    }
    /** Assign canonical ids to native and legacy Cube instances. */
    async ensureCubeIds(graph) {
        if (!graph)
            return { assigned: [], replacements: new Map() };
        const builder = this.options.instanceManager?.instanceBuilder ||
            new InstanceBuilder({ logger: this.options.adapter.getConsole?.() ?? null });
        const instances = builder.build(graph);
        const assigned = [];
        const replacements = new Map();
        const assignedIds = new Set();
        const reservedCubeIds = new Set((this.options.cubeBrowser?.getCubes?.() || [])
            .map((cube) => (typeof cube.cube_id === 'string' ? cube.cube_id.trim() : ''))
            .filter(Boolean));
        for (const instance of instances) {
            if (instance.cubeId && isCanonicalCubeId(instance.cubeId)) {
                reservedCubeIds.add(instance.cubeId);
            }
        }
        for (const instance of instances) {
            if (instance.cubeId && isCanonicalCubeId(instance.cubeId))
                continue;
            const defaultAlias = instance.defaultAlias || 'SugarCube';
            const cubeId = suggestPersonalCubeIdentity(defaultAlias, this.resolveTargetModel(instance.targetModel, instance.cubeId), Array.from(reservedCubeIds)).cubeId;
            reservedCubeIds.add(cubeId);
            if (instance.cubeId)
                replacements.set(instance.cubeId, cubeId);
            if (updateMarkersForIds(graph, instance.markerIds, { cubeId })) {
                assigned.push({ cubeId, instanceId: instance.instanceId });
                assignedIds.add(cubeId);
            }
        }
        const nodes = Array.isArray(graph._nodes) ? graph._nodes : graph.nodes || [];
        const fallbackByName = new Map();
        for (const node of nodes) {
            if (!isCubeMarkerType(node))
                continue;
            const defaultAliasValue = readWidgetValue(node, 'default_alias');
            const defaultAlias = typeof defaultAliasValue === 'string' ? defaultAliasValue : '';
            if (!defaultAlias)
                continue;
            const cubeIdValue = readWidgetValue(node, 'cube_id');
            const cubeId = typeof cubeIdValue === 'string' ? cubeIdValue : '';
            const entry = fallbackByName.get(defaultAlias) || {
                defaultAlias,
                cubeId: cubeId || '',
                markerIds: [],
            };
            if (cubeId && !entry.cubeId)
                entry.cubeId = cubeId;
            if (!cubeId && node.id != null)
                entry.markerIds.push(node.id);
            fallbackByName.set(defaultAlias, entry);
        }
        for (const entry of fallbackByName.values()) {
            if (!entry.markerIds.length)
                continue;
            const shouldReplace = Boolean(entry.cubeId && !isCanonicalCubeId(entry.cubeId));
            const cubeId = shouldReplace
                ? suggestPersonalCubeIdentity(entry.defaultAlias, this.resolveTargetModel(entry.cubeId), Array.from(reservedCubeIds)).cubeId
                : entry.cubeId;
            if (shouldReplace) {
                replacements.set(entry.cubeId, cubeId);
                reservedCubeIds.add(cubeId);
            }
            if (!cubeId) {
                const personalId = suggestPersonalCubeIdentity(entry.defaultAlias, this.resolveTargetModel(entry.cubeId), Array.from(reservedCubeIds)).cubeId;
                reservedCubeIds.add(personalId);
                if (updateMarkersForIds(graph, entry.markerIds, { cubeId: personalId })) {
                    this.recordAssignment(assigned, assignedIds, personalId);
                }
                continue;
            }
            if (updateMarkersForIds(graph, entry.markerIds, { cubeId })) {
                this.recordAssignment(assigned, assignedIds, cubeId);
            }
        }
        if (assigned.length) {
            this.options.instanceManager?.scheduleRefresh?.({ graph, reason: 'cube-id-assigned' });
            this.options.dirtyManager?.scheduleRefresh?.({ graph, reason: 'cube-id-assigned' });
        }
        return { assigned, replacements };
    }
    /** Resolve the first usable target model or the universal model. */
    resolveTargetModel(...candidates) {
        for (const candidate of candidates) {
            try {
                const direct = normalizeTargetModel(candidate);
                if (direct)
                    return direct;
            }
            catch (_error) {
                // A candidate Cube id is evaluated through its route below.
            }
            if (typeof candidate === 'string' && candidate.trim()) {
                try {
                    const fromCubeId = normalizeTargetModel(deriveTargetModelFromCubeId(candidate));
                    if (fromCubeId)
                        return fromCubeId;
                }
                catch (_error) {
                    // Continue to the next candidate.
                }
            }
        }
        return ANY_TARGET_MODEL;
    }
    recordAssignment(assigned, assignedIds, cubeId) {
        if (!assignedIds.has(cubeId)) {
            assigned.push({ cubeId, instanceId: null });
            assignedIds.add(cubeId);
        }
    }
}
