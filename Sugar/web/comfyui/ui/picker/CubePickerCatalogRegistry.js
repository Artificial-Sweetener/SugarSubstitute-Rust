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
/** Reconcile picker descriptors and placement-ready payloads as one atomic snapshot. */
import { readImportPayload } from '../import/PlacementPayload.js';
import { isRecord } from '../types/common.js';
import { projectComfyCubeNodeDef, } from './ComfyCubeNodeDefProjector.js';
import { readCubePickerCatalog } from './CubePickerDescriptor.js';
const EMPTY_SNAPSHOT = {
    revision: '',
    entriesByType: new Map(),
};
/** Own catalog revision, prepared payload availability, and atomic reconciliation. */
export class CubePickerCatalogRegistry {
    #api;
    #logger;
    #snapshot = EMPTY_SNAPSHOT;
    #refreshing = null;
    #queuedForcedRefresh = null;
    /** Bind the narrow Sugar API and structured diagnostic boundary. */
    constructor(options) {
        this.#api = options.api;
        this.#logger = options.logger;
    }
    /** Return the current successfully reconciled catalog revision. */
    get revision() {
        return this.#snapshot.revision;
    }
    /** Refresh once, coalescing callers so definitions never observe a partial cache. */
    refresh(options = {}) {
        if (this.#refreshing) {
            if (options.force !== true)
                return this.#refreshing;
            if (this.#queuedForcedRefresh)
                return this.#queuedForcedRefresh;
            const resume = () => {
                this.#queuedForcedRefresh = null;
                return this.refresh({ force: true });
            };
            const queued = this.#refreshing.then(resume, resume);
            this.#queuedForcedRefresh = queued;
            return queued;
        }
        const operation = this.#refresh(options.force === true).finally(() => {
            if (this.#refreshing === operation)
                this.#refreshing = null;
        });
        this.#refreshing = operation;
        return operation;
    }
    /** Project fresh node definitions from the current host-neutral descriptors. */
    definitions() {
        return [...this.#snapshot.entriesByType.values()].map(({ descriptor }) => projectComfyCubeNodeDef(descriptor));
    }
    /** Return the current immutable descriptor view with each reserved Comfy type. */
    entries() {
        return [...this.#snapshot.entriesByType.entries()].map(([type, { descriptor }]) => ({
            type,
            descriptor,
        }));
    }
    /** Return the descriptor currently advertised for one reserved Comfy type. */
    descriptor(type) {
        return this.#snapshot.entriesByType.get(type)?.descriptor ?? null;
    }
    /** Return an isolated payload copy so each construction owns its mutations. */
    preparedPayload(type) {
        const payload = this.#snapshot.entriesByType.get(type)?.payload;
        if (!payload)
            return null;
        return readImportPayload(cloneJsonRecord(payload));
    }
    /** Retain a frozen validated artifact for a cart's exact catalog generation. */
    capturedPayload(type) {
        return this.#snapshot.entriesByType.get(type)?.payload ?? null;
    }
    /** Reuse unchanged artifacts and publish validated changes as one snapshot. */
    async #refresh(force) {
        const catalogResult = await withPickerRequestTimeout((signal) => this.#api.listPickerCatalog({ cache: 'no-store', signal }));
        requireSuccessfulResponse(catalogResult, 'Cube picker catalog request failed');
        const catalog = readCubePickerCatalog(catalogResult.data);
        if (!catalog)
            throw new Error('Cube picker catalog response is incompatible.');
        if (catalog.catalogRevision === this.#snapshot.revision &&
            catalog.entries.length === this.#snapshot.entriesByType.size) {
            return this.#result(false, []);
        }
        for (const error of catalog.errors) {
            this.#logger.warn('SugarCubes picker catalog omitted a Cube.', {
                cubeId: error.cubeId,
                reason: error.message,
            });
        }
        const knownCubeIds = new Set([...this.#snapshot.entriesByType.values()].map(({ descriptor }) => descriptor.cubeId));
        if (catalog.errors.some((error) => knownCubeIds.has(error.cubeId))) {
            throw new Error('Cube picker refresh could not validate an existing Cube. The previous catalog is retained.');
        }
        const outcomes = await Promise.all(catalog.entries.map(async (descriptor) => {
            try {
                const previous = this.#snapshot.entriesByType.get(projectComfyCubeNodeDef(descriptor).name);
                if (descriptor.contentHash &&
                    previous?.descriptor.contentHash === descriptor.contentHash &&
                    previous.descriptor.cubeId === descriptor.cubeId &&
                    JSON.stringify(previous.descriptor.source) === JSON.stringify(descriptor.source)) {
                    freezeJsonValue(descriptor);
                    return { descriptor, payload: previous.payload };
                }
                return await this.#loadEntry(descriptor);
            }
            catch (error) {
                this.#logger.error('SugarCubes picker payload is unavailable.', {
                    cubeId: descriptor.cubeId,
                    reason: readErrorMessage(error),
                    error,
                });
                return descriptor.cubeId;
            }
        }));
        const entriesByType = new Map();
        const unavailableCubeIds = [];
        for (const outcome of outcomes) {
            if (typeof outcome === 'string') {
                unavailableCubeIds.push(outcome);
                continue;
            }
            const definition = projectComfyCubeNodeDef(outcome.descriptor);
            entriesByType.set(definition.name, outcome);
        }
        if (unavailableCubeIds.length && this.#snapshot.revision) {
            throw new Error(`Cube picker refresh could not validate: ${unavailableCubeIds.join(', ')}.`);
        }
        this.#snapshot = { revision: catalog.catalogRevision, entriesByType };
        this.#logger.debug('SugarCubes picker catalog reconciled.', {
            revision: catalog.catalogRevision,
            advertisedCount: entriesByType.size,
            unavailableCount: unavailableCubeIds.length,
            forced: force,
        });
        return this.#result(true, unavailableCubeIds);
    }
    /** Fetch and validate one placement payload before its definition becomes visible. */
    async #loadEntry(descriptor) {
        const result = await withPickerRequestTimeout((signal) => this.#api.load(JSON.stringify({
            cube_id: descriptor.cubeId,
            version_pin: descriptor.version,
            origin: { x: 0, y: 0 },
        }), { headers: { 'Content-Type': 'application/json' }, signal }));
        requireSuccessfulResponse(result, `Cube '${descriptor.cubeId}' could not be loaded`);
        const payload = readImportPayload(result.data);
        if (!payload || !isRecord(payload.cube)) {
            throw new Error(`Cube '${descriptor.cubeId}' returned an invalid placement payload.`);
        }
        const payloadCubeId = readString(payload.cube.cube_id);
        const payloadVersion = readString(payload.cube.version);
        if (payloadCubeId !== descriptor.cubeId) {
            throw new Error(`Cube '${descriptor.cubeId}' returned mismatched identity '${payloadCubeId}'.`);
        }
        if (descriptor.version && payloadVersion !== descriptor.version) {
            throw new Error(`Cube '${descriptor.cubeId}' returned version '${payloadVersion}' instead of '${descriptor.version}'.`);
        }
        if (descriptor.contentHash && payload.contentHash !== descriptor.contentHash) {
            throw new Error(`Cube '${descriptor.cubeId}' changed while its picker payload was loading.`);
        }
        freezeJsonValue(descriptor);
        freezeJsonValue(payload);
        return { descriptor, payload };
    }
    /** Describe the current snapshot without exposing mutable registry state. */
    #result(changed, unavailableCubeIds) {
        return {
            changed,
            revision: this.#snapshot.revision,
            definitions: this.definitions(),
            unavailableCubeIds: [...unavailableCubeIds],
        };
    }
}
/** Freeze JSON artifact trees once so consumers can retain revisions without copying the library. */
function freezeJsonValue(value) {
    if (!Array.isArray(value) && !isRecord(value))
        return;
    for (const child of Object.values(value))
        freezeJsonValue(child);
    Object.freeze(value);
}
/** Bound external catalog requests and release each deadline after settlement. */
async function withPickerRequestTimeout(request) {
    const controller = new AbortController();
    const deadline = setTimeout(() => controller.abort(), 15_000);
    try {
        return await request(controller.signal);
    }
    finally {
        clearTimeout(deadline);
    }
}
/** Reject unsuccessful HTTP responses and backend error envelopes. */
function requireSuccessfulResponse(result, fallback) {
    const backendError = isRecord(result.data.error) ? result.data.error : null;
    if (result.response.ok === true && !backendError)
        return;
    const message = readString(backendError?.message) || result.response.statusText || fallback;
    throw new Error(message);
}
/** Clone JSON-safe prepared data before returning it to construction code. */
function cloneJsonRecord(value) {
    const parsed = JSON.parse(JSON.stringify(value));
    if (!isRecord(parsed))
        throw new TypeError('Cube picker payload could not be cloned.');
    return parsed;
}
/** Read one optional dynamic string. */
function readString(value) {
    return typeof value === 'string' ? value.trim() : '';
}
/** Normalize one caught error for structured user diagnostics. */
function readErrorMessage(error) {
    if (error instanceof Error && error.message.trim())
        return error.message.trim();
    return String(error || 'Unknown picker payload failure');
}
