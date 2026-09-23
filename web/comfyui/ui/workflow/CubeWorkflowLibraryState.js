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
/** Own non-persisted library classification for the active workflow. */
import { isRecord } from '../types/common.js';
/** Keep machine-derived classification out of serialized workflow properties. */
export class CubeWorkflowLibraryState {
    #api;
    #logger;
    #listeners = new Set();
    #byInstanceId = new Map();
    #generation = 0;
    /** Bind the versioned SugarCubes API and a host diagnostic sink. */
    constructor(api, logger) {
        this.#api = api;
        this.#logger = logger;
    }
    /** Start best-effort enrichment without blocking Comfy graph hydration. */
    begin(workflow) {
        const generation = ++this.#generation;
        this.#replace([]);
        void this.#classify(workflow, generation).catch((error) => {
            if (generation !== this.#generation)
                return;
            this.#logger.warn('SugarCubes: workflow Cube classification unavailable', error);
        });
    }
    /** Read current machine state for one stable instance identity. */
    read(instanceId) {
        return this.#byInstanceId.get(instanceId) ?? null;
    }
    /** Return whether definition editing is currently authorized for one instance. */
    canEdit(instanceId) {
        const classification = this.read(instanceId);
        return classification ? classification.access === 'writable' : null;
    }
    /** Observe classification replacement for badges and workflow actions. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => this.#listeners.delete(listener);
    }
    /** Cancel stale requests and clear all machine-derived state. */
    clear() {
        this.#generation += 1;
        this.#replace([]);
    }
    /** Submit an immutable workflow snapshot and apply only the latest response. */
    async #classify(workflow, generation) {
        const result = await this.#api.classifyWorkflow(JSON.stringify({ workflow }), {
            headers: { 'Content-Type': 'application/json' },
        });
        if (!result.response.ok) {
            throw new Error(`Workflow Cube classification failed (${String(result.response.status)}).`);
        }
        const definitions = readClassificationResponse(result.data);
        if (generation === this.#generation)
            this.#replace(definitions);
    }
    /** Atomically replace the instance index and notify presentation subscribers. */
    #replace(definitions) {
        const next = new Map();
        for (const definition of definitions) {
            for (const instanceId of definition.instanceIds)
                next.set(instanceId, definition);
        }
        this.#byInstanceId = next;
        for (const listener of this.#listeners)
            listener();
    }
}
/** Validate the versioned response before it becomes active UI state. */
function readClassificationResponse(value) {
    if (!isRecord(value) || value.schema_version !== 1 || !Array.isArray(value.definitions)) {
        throw new TypeError('Workflow Cube classification response is invalid.');
    }
    return value.definitions.map((entry, index) => readClassification(entry, index));
}
/** Parse one complete definition classification from an untrusted response. */
function readClassification(value, index) {
    if (!isRecord(value))
        throw new TypeError(`Workflow Cube classification #${String(index)} is invalid.`);
    const primaryClass = requireEnum(value.primary_class, ['none', 'captured', 'local', 'synced'], 'primary_class');
    const access = requireEnum(value.access, ['read_only', 'writable'], 'access');
    if (!Array.isArray(value.instance_ids) ||
        value.instance_ids.some((item) => typeof item !== 'string')) {
        throw new TypeError('Workflow Cube classification instance_ids must be strings.');
    }
    if (!Array.isArray(value.permitted_operations) ||
        value.permitted_operations.some((item) => typeof item !== 'string')) {
        throw new TypeError('Workflow Cube classification operations must be strings.');
    }
    return {
        definitionId: requireString(value.definition_id, 'definition_id'),
        cubeId: requireString(value.cube_id, 'cube_id'),
        cubeVersion: requireString(value.cube_version, 'cube_version'),
        semanticHash: requireString(value.semantic_hash, 'semantic_hash'),
        instanceIds: value.instance_ids,
        primaryClass,
        access,
        sourceAvailable: value.source_available === true,
        permittedOperations: new Set(value.permitted_operations),
    };
}
/** Require one non-empty API string. */
function requireString(value, field) {
    const normalized = typeof value === 'string' ? value.trim() : '';
    if (!normalized)
        throw new TypeError(`Workflow Cube classification ${field} is required.`);
    return normalized;
}
/** Narrow one response string to an explicit protocol enum. */
function requireEnum(value, allowed, field) {
    if (typeof value === 'string' && allowed.includes(value))
        return value;
    throw new TypeError(`Workflow Cube classification ${field} is invalid.`);
}
