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
 * Own the SugarCubes local flavor persistence layer in
 * `web/comfyui/ui/flavors/FlavorStorage.js`.
 */
import { isRecord } from '../types/common.js';
const LOCAL_FLAVOR_SCHEMA = 1;
const STORAGE_KEY_PREFIX = 'sugarcubes.local_flavors.';
const MIGRATION_MARKER_KEY = 'sugarcubes.local_flavors_migrated.v1';
function cloneValue(value) {
    return JSON.parse(JSON.stringify(isRecord(value) ? value : {}));
}
function normalizeCubeId(cubeId) {
    return String(cubeId || '').trim();
}
function normalizeState(cubeId, state) {
    const source = isRecord(state) ? state : {};
    const normalizedCubeId = normalizeCubeId(source.cube_id || cubeId);
    return {
        schema_version: LOCAL_FLAVOR_SCHEMA,
        cube_id: normalizedCubeId,
        surfaces: normalizeSurfaces(source.surfaces),
        updated_at: typeof source.updated_at === 'string' ? source.updated_at : '',
    };
}
function normalizeSurfaces(value) {
    if (!isRecord(value))
        return {};
    return Object.fromEntries(Object.entries(value).map(([signature, surfaceValue]) => {
        const surface = isRecord(surfaceValue) ? surfaceValue : {};
        return [
            signature,
            {
                selected_flavor_id: typeof surface.selected_flavor_id === 'string' ? surface.selected_flavor_id : '',
                flavors: Array.isArray(surface.flavors)
                    ? surface.flavors.filter((entry) => isRecord(entry))
                    : [],
            },
        ];
    }));
}
function readResponseError(response, data, fallback) {
    const error = isRecord(data.error) ? data.error : {};
    return ((typeof error.message === 'string' ? error.message : '') || response?.statusText || fallback);
}
/**
 * Coordinate API-backed persistence and cached reads for private local flavors.
 */
export class FlavorStorage {
    storage;
    api;
    cache;
    constructor({ storage = null, api = null } = {}) {
        this.storage = storage;
        this.api = api;
        this.cache = new Map();
    }
    buildStorageKey(cubeId) {
        return `${STORAGE_KEY_PREFIX}${normalizeCubeId(cubeId)}`;
    }
    readCubeState(cubeId) {
        const key = normalizeCubeId(cubeId);
        return normalizeState(key, this.cache.get(key));
    }
    writeCubeState(cubeId, state) {
        const key = normalizeCubeId(cubeId);
        this.cache.set(key, normalizeState(key, state));
    }
    async loadCubeState(cubeId) {
        const key = normalizeCubeId(cubeId);
        if (!key) {
            return normalizeState('', null);
        }
        if (!this.api?.getLocalFlavors) {
            const state = normalizeState(key, this.cache.get(key));
            this.cache.set(key, state);
            return state;
        }
        const { response, data } = await this.api.getLocalFlavors(key);
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor load failed'));
        }
        const state = normalizeState(key, data?.state);
        this.cache.set(key, state);
        return state;
    }
    readSurfaceState(cubeId, surfaceSignature) {
        const state = this.readCubeState(cubeId);
        const key = String(surfaceSignature || '').trim();
        return state.surfaces[key] ?? { selected_flavor_id: '', flavors: [] };
    }
    async saveLocalFlavor({ cubeId, surfaceSignature, name, values, flavorId, authoredFlavors = [], } = {}) {
        const signature = String(surfaceSignature || '').trim();
        if (!signature) {
            return null;
        }
        if (!this.api?.saveLocalFlavor) {
            throw new Error('Local flavor API unavailable');
        }
        const payload = {
            cube_id: normalizeCubeId(cubeId),
            surface_signature: signature,
            name: String(name || '').trim(),
            values: cloneValue(values),
            flavor_id: String(flavorId || '').trim(),
            authored_flavors: Array.isArray(authoredFlavors) ? authoredFlavors : [],
        };
        const { response, data } = await this.api.saveLocalFlavor(JSON.stringify(payload), {
            headers: { 'Content-Type': 'application/json' },
        });
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor save failed'));
        }
        const state = normalizeState(payload.cube_id, data?.state);
        this.cache.set(payload.cube_id, state);
        const surfaceState = this.readSurfaceState(payload.cube_id, signature);
        return (surfaceState.flavors.find((entry) => entry?.id === surfaceState.selected_flavor_id) ||
            surfaceState.flavors[surfaceState.flavors.length - 1] ||
            null);
    }
    async deleteLocalFlavor({ cubeId, surfaceSignature, flavorId, } = {}) {
        const key = normalizeCubeId(cubeId);
        const signature = String(surfaceSignature || '').trim();
        const targetId = String(flavorId || '').trim();
        if (!key || !signature || !targetId) {
            return false;
        }
        if (!this.api?.deleteLocalFlavor) {
            throw new Error('Local flavor API unavailable');
        }
        const beforeCount = this.readSurfaceState(key, signature).flavors.length;
        const { response, data } = await this.api.deleteLocalFlavor(JSON.stringify({
            cube_id: key,
            surface_signature: signature,
            flavor_id: targetId,
        }), { headers: { 'Content-Type': 'application/json' } });
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor delete failed'));
        }
        const state = normalizeState(key, data?.state);
        this.cache.set(key, state);
        const afterCount = this.readSurfaceState(key, signature).flavors.length;
        return afterCount !== beforeCount;
    }
    async setSelectedFlavorId({ cubeId, surfaceSignature, flavorId, } = {}) {
        const key = normalizeCubeId(cubeId);
        const signature = String(surfaceSignature || '').trim();
        if (!key || !signature) {
            return;
        }
        if (!this.api?.selectLocalFlavor) {
            throw new Error('Local flavor API unavailable');
        }
        const { response, data } = await this.api.selectLocalFlavor(JSON.stringify({
            cube_id: key,
            surface_signature: signature,
            flavor_id: String(flavorId || '').trim(),
        }), { headers: { 'Content-Type': 'application/json' } });
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor selection failed'));
        }
        this.cache.set(key, normalizeState(key, data?.state));
    }
    async reconcileLocalFlavors({ cubeId, surfaceSignature, authoredFlavors, renameMap, } = {}) {
        const key = normalizeCubeId(cubeId);
        const signature = String(surfaceSignature || '').trim();
        if (!key || !signature || !this.api?.reconcileLocalFlavors) {
            return { conflict_count: 0, renamed: [], state: this.readCubeState(key) };
        }
        const { response, data } = await this.api.reconcileLocalFlavors(JSON.stringify({
            cube_id: key,
            surface_signature: signature,
            authored_flavors: Array.isArray(authoredFlavors) ? authoredFlavors : [],
            rename_map: isRecord(renameMap) ? renameMap : {},
        }), { headers: { 'Content-Type': 'application/json' } });
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor reconciliation failed'));
        }
        const state = normalizeState(key, data?.state);
        this.cache.set(key, state);
        return { ...data, state };
    }
    async migrateBrowserStorage() {
        if (!this.api?.migrateLocalFlavors || !this.storage) {
            return { count: 0 };
        }
        if (this.storage.readValue?.(MIGRATION_MARKER_KEY)) {
            return { count: 0 };
        }
        const storage = this.storage.getStorage?.() || null;
        if (!storage || typeof storage.length !== 'number' || typeof storage.key !== 'function') {
            return { count: 0 };
        }
        const states = [];
        for (let index = 0; index < storage.length; index += 1) {
            const key = storage.key(index);
            if (typeof key !== 'string' || !key.startsWith(STORAGE_KEY_PREFIX)) {
                continue;
            }
            const cubeId = key.slice(STORAGE_KEY_PREFIX.length);
            const state = this.storage.readJson?.(key);
            if (!cubeId || !state || typeof state !== 'object') {
                continue;
            }
            states.push({ cube_id: cubeId, state });
        }
        if (!states.length) {
            this.storage.writeValue?.(MIGRATION_MARKER_KEY, new Date().toISOString());
            return { count: 0 };
        }
        const { response, data } = await this.api.migrateLocalFlavors(JSON.stringify({ states }), {
            headers: { 'Content-Type': 'application/json' },
        });
        if (!response?.ok || data?.error) {
            throw new Error(readResponseError(response, data, 'Local flavor migration failed'));
        }
        this.storage.writeValue?.(MIGRATION_MARKER_KEY, new Date().toISOString());
        return data;
    }
}
