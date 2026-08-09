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
/** Own version-history and exact Cube artifact access at the backend boundary. */
import { isCurrentRevisionRef } from '../../core/CubeDefinitionKey.js';
import { readImportPayload } from '../../import/PlacementPayload.js';
import { isRecord } from '../../types/common.js';
import { projectCubeVersionOptions } from './CubeVersionOptionProjection.js';
/** Retrieve validated version options and exact prepared artifacts. */
export class CubeVersionRepository {
    #api;
    #logger;
    #artifacts = new Map();
    /** Bind the typed SugarCubes API boundary. */
    constructor(api, logger = console) {
        this.#api = api;
        this.#logger = logger;
    }
    /** List unique semantic versions whose exact artifacts pass loader validation. */
    async listOptions(cubeId, fallbackVersion) {
        const requiredCubeId = requireCubeId(cubeId);
        const { response, data } = await this.#api.listRevisions(requiredCubeId);
        if (!response.ok || data.error)
            throw new Error(readApiError(data) || 'Cube versions unavailable.');
        const revisions = Array.isArray(data.version_revisions)
            ? data.version_revisions.filter(isRecord).map((value) => value)
            : [];
        const options = projectCubeVersionOptions(revisions, fallbackVersion);
        const available = await Promise.all(options.map(async (option) => {
            try {
                await this.#loadCachedArtifact(requiredCubeId, option);
                return option;
            }
            catch (error) {
                this.#logger.warn(`SugarCubes: omitted unavailable Cube version '${option.label}' for '${requiredCubeId}'.`, error);
                return null;
            }
        }));
        return available.filter((option) => option !== null);
    }
    /** Load the exact artifact represented by one version option. */
    async loadArtifact(cubeId, option) {
        return this.#loadCachedArtifact(requireCubeId(cubeId), option);
    }
    /** Coalesce loader validation and preserve prepared immutable artifacts for switching. */
    #loadCachedArtifact(cubeId, option) {
        const key = `${cubeId}\u0000${option.value}\u0000${option.revisionRef}`;
        const existing = this.#artifacts.get(key);
        if (existing)
            return existing;
        const request = this.#loadArtifact(cubeId, option).catch((error) => {
            this.#artifacts.delete(key);
            throw error;
        });
        this.#artifacts.set(key, request);
        return request;
    }
    /** Retrieve and validate one exact prepared artifact from the backend. */
    async #loadArtifact(cubeId, option) {
        const body = isCurrentRevisionRef(option.revisionRef)
            ? { cube_id: cubeId, origin: { x: 0, y: 0 } }
            : {
                cube_id: cubeId,
                revision_ref: option.revisionRef,
                version_pin: option.value,
                origin: { x: 0, y: 0 },
            };
        const loader = isCurrentRevisionRef(option.revisionRef)
            ? this.#api.load.bind(this.#api)
            : this.#api.loadRevision.bind(this.#api);
        const { response, data } = await loader(JSON.stringify(body), {
            headers: { 'Content-Type': 'application/json' },
        });
        if (!response.ok || data.error)
            throw new Error(readApiError(data) || 'Cube version failed to load.');
        const payload = readImportPayload(data);
        if (!payload)
            throw new Error('Cube version returned an invalid artifact.');
        return payload;
    }
}
/** Require one durable Cube identity before crossing the API boundary. */
function requireCubeId(value) {
    const cubeId = value.trim();
    if (!cubeId)
        throw new TypeError('Cube identity is required for version access.');
    return cubeId;
}
/** Read one actionable backend error without trusting arbitrary response values. */
function readApiError(data) {
    if (typeof data.error === 'string')
        return data.error.trim();
    if (!isRecord(data.error))
        return '';
    const message = typeof data.error.message === 'string' ? data.error.message.trim() : '';
    const detail = typeof data.error.detail === 'string' ? data.error.detail.trim() : '';
    return [message, detail].filter(Boolean).join(' ');
}
