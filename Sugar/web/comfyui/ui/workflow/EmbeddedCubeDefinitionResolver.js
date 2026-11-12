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
/** Resolve validated workflow-embedded definitions without consulting a catalog. */
import { buildCubeDefinitionKey, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import { parseCanonicalCubeId } from '../core/CubeId.js';
import { hashText, stableStringify } from '../graph/DirtyHasher.js';
import { isRecord } from '../types/common.js';
const CUBE_KINDS = new Set(['cube', 'cube_draft']);
/** Own strict recognition of persisted Cube markers and their embedded definitions. */
export class EmbeddedCubeDefinitionResolver {
    /** Resolve every persisted Cube definition used by the native workflow. */
    resolve(workflow) {
        if (!isRecord(workflow))
            return [];
        const definitions = isRecord(workflow.definitions) ? workflow.definitions.subgraphs : undefined;
        const nodes = Array.isArray(workflow.nodes) ? workflow.nodes : [];
        if (!Array.isArray(definitions)) {
            requireNoPersistedCubeInstances(nodes, new Map());
            return [];
        }
        const byDefinitionId = new Map();
        for (const [index, candidate] of definitions.entries()) {
            if (!isRecord(candidate) || !isRecord(candidate.extra))
                continue;
            if (!CUBE_KINDS.has(String(candidate.extra.sugarcubes_kind)))
                continue;
            const path = `definitions.subgraphs[${String(index)}]`;
            const definitionId = requireString(candidate.id, `${path}.id`);
            if (byDefinitionId.has(definitionId)) {
                throw new TypeError(`Duplicate embedded Cube definition '${definitionId}'.`);
            }
            const identity = requireRecord(candidate.extra.sugarcubes_cube, `${path}.extra.sugarcubes_cube`);
            const cubeId = requireString(identity.cube_id, `${path}.extra.sugarcubes_cube.cube_id`);
            try {
                parseCanonicalCubeId(cubeId);
            }
            catch (error) {
                throw new TypeError(`Workflow value '${path}.extra.sugarcubes_cube.cube_id' must be a canonical Cube id.`, {
                    cause: error,
                });
            }
            const cubeVersion = requireString(identity.cube_version, `${path}.extra.sugarcubes_cube.cube_version`);
            const serialized = stableStringify(candidate);
            if (!serialized)
                throw new TypeError(`Embedded Cube definition '${definitionId}' is invalid.`);
            const revisionRef = normalizeRevisionRef(identity.cube_revision_ref);
            byDefinitionId.set(definitionId, {
                cubeId,
                cubeVersion,
                revisionRef,
                definitionKey: buildCubeDefinitionKey(cubeId, cubeVersion),
                definitionId,
                contentFingerprint: `embedded:${hashText(serialized)}`,
                payload: cloneRecord(candidate),
            });
        }
        requireNoPersistedCubeInstances(nodes, byDefinitionId);
        return Array.from(byDefinitionId.values()).sort((left, right) => left.definitionId.localeCompare(right.definitionId));
    }
}
/** Validate every persisted instance against its workflow-owned definition. */
function requireNoPersistedCubeInstances(nodes, definitions) {
    const instanceIds = new Set();
    for (const [index, candidate] of nodes.entries()) {
        if (!isRecord(candidate) || !isRecord(candidate.properties))
            continue;
        if (!CUBE_KINDS.has(String(candidate.properties.sugarcubes_kind)))
            continue;
        const path = `nodes[${String(index)}]`;
        const definitionId = requireString(candidate.type, `${path}.type`);
        const definition = definitions.get(definitionId);
        if (!definition) {
            throw new TypeError(`Cube node at '${path}' is missing embedded definition '${definitionId}'.`);
        }
        const identity = requireRecord(candidate.properties.sugarcubes_cube, `${path}.properties.sugarcubes_cube`);
        const instanceId = requireString(identity.instance_id, `${path}.properties.sugarcubes_cube.instance_id`);
        if (instanceIds.has(instanceId))
            throw new TypeError(`Duplicate Cube instance '${instanceId}'.`);
        instanceIds.add(instanceId);
        const cubeId = readString(identity.cube_id) || definition.cubeId;
        const cubeVersion = readString(identity.cube_version) || definition.cubeVersion;
        if (cubeId !== definition.cubeId || cubeVersion !== definition.cubeVersion) {
            throw new TypeError(`Cube instance '${instanceId}' disagrees with its embedded definition.`);
        }
    }
}
/** Require one dynamic value to be a record. */
function requireRecord(value, path) {
    if (!isRecord(value))
        throw new TypeError(`Workflow value '${path}' must be an object.`);
    return value;
}
/** Require one dynamic value to be non-empty text. */
function requireString(value, path) {
    const normalized = readString(value);
    if (!normalized)
        throw new TypeError(`Workflow value '${path}' must be a non-empty string.`);
    return normalized;
}
/** Read optional normalized text. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Clone one JSON-domain definition before assigning cache ownership. */
function cloneRecord(value) {
    const cloned = JSON.parse(JSON.stringify(value));
    return isRecord(cloned) ? cloned : {};
}
