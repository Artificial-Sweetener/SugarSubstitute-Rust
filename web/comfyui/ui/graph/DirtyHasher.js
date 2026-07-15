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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtyHasher.js`.
 */
import { sanitizeValue } from './DirtySnapshotter.js';
import { snapshotImplementationDefinition, snapshotImplementationInstance, } from './ImplementationSnapshotter.js';
import { snapshotSurfaceInstance, snapshotSurfaceValues } from './SurfaceSnapshotter.js';
import { snapshotCosmeticInstance } from './CosmeticSnapshotter.js';
import { isRecord } from '../types/common.js';
/**
 * Hash text.
 */
export function hashText(text) {
    let hash = 5381;
    const value = String(text || '');
    for (let i = 0; i < value.length; i += 1) {
        hash = (hash * 33) ^ value.charCodeAt(i);
    }
    return (hash >>> 0).toString(36);
}
/**
 * Build stable stringify.
 */
export function stableStringify(value) {
    const seen = new WeakSet();
    const sanitized = sanitizeValue(value, seen, 0);
    return JSON.stringify(sanitized);
}
/**
 * Compute definition hash.
 */
export function computeDefinitionHash(definition) {
    const record = isRecord(definition) ? definition : {};
    const cube = isRecord(record.cube) ? record.cube : {};
    const surface = isRecord(cube.surface) ? cube.surface : null;
    const snapshot = snapshotImplementationDefinition(definition, surface);
    if (!snapshot) {
        return null;
    }
    return hashText(stableStringify(snapshot));
}
/**
 * Compute instance hash.
 */
export function computeInstanceHash(graph, nodeIds, markerIds, anchor, groupSnapshot, options = {}) {
    const payload = groupSnapshot && groupSnapshot.__surface_snapshot
        ? snapshotSurfaceValues(groupSnapshot.surface, groupSnapshot.values)
        : snapshotImplementationInstance(graph, nodeIds, markerIds, anchor, options.surface || null, options);
    return hashText(stableStringify(payload));
}
/**
 * Compute implementation hash.
 */
export function computeImplementationHash(graph, nodeIds, markerIds, anchor, surface, options = {}) {
    return hashText(stableStringify(snapshotImplementationInstance(graph, nodeIds, markerIds, anchor, surface, options)));
}
/**
 * Compute cosmetic hash.
 */
export function computeCosmeticHash(graph, nodeIds, markerIds, anchor, group) {
    return hashText(stableStringify(snapshotCosmeticInstance(graph, nodeIds, markerIds, anchor, group)));
}
/**
 * Compute surface hash from live nodes.
 */
export function computeSurfaceHash(graph, nodeIds, surface) {
    return hashText(stableStringify(snapshotSurfaceInstance(graph, nodeIds, surface)));
}
/**
 * Compute baseline surface hash from persisted flavor values.
 */
export function computeSurfaceValuesHash(surface, values) {
    return hashText(stableStringify(snapshotSurfaceValues(surface, values)));
}
