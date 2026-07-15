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
 * Reconcile the live graph to authoritative definitions returned by cube saves.
 */
import { buildCubeDefinitionKey } from '../core/CubeDefinitionKey.js';
import { updateMarkersForIds } from '../graph/CubeMarkers.js';
import { getGraphGroups } from '../graph/GraphQuery.js';
import { flattenCubeGroupMetadata, getGroupSugarcubes, setGroupSugarcubes, writeCubeDefinitionMetadata, } from '../graph/GroupMetadata.js';
import { isRecord } from '../types/common.js';
const WORKTREE_REVISION = 'WORKTREE';
/** Coordinate post-save definition, instance, preset, and dirty-state updates. */
export class CubeSaveReconciler {
    definitionStore;
    instanceManager;
    flavorService;
    dirtyManager;
    constructor({ definitionStore, instanceManager = null, flavorService = null, dirtyManager = null, }) {
        this.definitionStore = definitionStore;
        this.instanceManager = instanceManager;
        this.flavorService = flavorService;
        this.dirtyManager = dirtyManager;
    }
    /** Reconcile successful save results before control returns to the caller. */
    async reconcile({ graph, saved, fallbackCubeIds = [], markerIdsByCubeId = {}, reason = 'cube-save', }) {
        const finalized = this.publishDefinitions(graph, saved, markerIdsByCubeId);
        const savedCubeIds = finalized.map(({ cubeId }) => cubeId);
        const cubeIds = savedCubeIds.length ? savedCubeIds : normalizeCubeIds(fallbackCubeIds);
        if (!finalized.length && cubeIds.length) {
            throw new Error('Cube save response is missing finalized definitions');
        }
        this.instanceManager?.refresh?.({ graph, reason });
        for (const result of finalized) {
            await this.flavorService?.hydrateFromDefinition?.({
                cubeId: result.cubeId,
                definitionKey: result.definitionKey,
                entry: result.entry,
                graph,
                forceApply: true,
            });
        }
        this.dirtyManager?.acceptFinalizedDefinitions?.({ graph, entries: finalized });
        this.dirtyManager?.addSavedIds?.(cubeIds);
        this.dirtyManager?.markClean?.({ graph, cubeIds });
        return { cubeIds, entries: finalized };
    }
    /** Publish each persisted definition and align marker definition identity. */
    publishDefinitions(graph, saved, markerIdsByCubeId) {
        const results = [];
        for (const savedEntry of Array.isArray(saved) ? saved : []) {
            const definition = isRecord(savedEntry.definition) ? savedEntry.definition : null;
            const definitionCube = isRecord(definition?.cube) ? definition.cube : null;
            const cubeId = readString(savedEntry.cube_id) || readString(definitionCube?.cube_id);
            const cubeVersion = readString(definitionCube?.version) || readString(savedEntry.version);
            if (!cubeId || !definition || !definitionCube) {
                continue;
            }
            const definitionKey = buildCubeDefinitionKey(cubeId, cubeVersion);
            const markerIds = markerIdsByCubeId[cubeId] ?? [];
            if (!markerIds.length) {
                throw new Error(`Cube save reconciliation targets are missing for '${cubeId}'`);
            }
            updateMarkersForIds(graph, markerIds, {
                cubeVersion,
                cubeRevisionRef: WORKTREE_REVISION,
            });
            this.alignTargetGroupIdentity({
                graph,
                cubeId,
                cubeVersion,
                definitionKey,
                markerIds,
            });
            const entry = this.definitionStore.publishFinalized({
                cubeId,
                cubeVersion,
                revisionRef: WORKTREE_REVISION,
                definitionKey,
            }, definition);
            results.push({ cubeId, cubeVersion, definitionKey, entry });
        }
        return results;
    }
    /** Align only groups whose markers supplied this save. */
    alignTargetGroupIdentity({ graph, cubeId, cubeVersion, definitionKey, markerIds, }) {
        const targetMarkerIds = new Set(markerIds.map(String));
        for (const group of getGraphGroups(graph)) {
            const metadata = getGroupSugarcubes(group);
            if (!metadata?.managed || metadata.cube_id !== cubeId) {
                continue;
            }
            const groupMarkerIds = readMetadataMarkerIds(metadata);
            if (!groupMarkerIds.some((markerId) => targetMarkerIds.has(markerId))) {
                continue;
            }
            const definitionMetadata = writeCubeDefinitionMetadata(metadata, {
                cube_version: cubeVersion,
                cube_revision_ref: WORKTREE_REVISION,
                cube_definition_key: definitionKey,
            });
            setGroupSugarcubes(group, flattenCubeGroupMetadata(definitionMetadata, metadata));
        }
    }
}
function normalizeCubeIds(values) {
    return Array.from(new Set(values.map(readString).filter(Boolean)));
}
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
function readMetadataMarkerIds(metadata) {
    if (Array.isArray(metadata.markers)) {
        return metadata.markers.map(String);
    }
    const markers = metadata.markers;
    if (!isRecord(markers)) {
        return [];
    }
    return Object.values(markers).flatMap((values) => Array.isArray(values) ? values.map(String) : []);
}
