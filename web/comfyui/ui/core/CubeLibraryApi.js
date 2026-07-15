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
 * Own the SugarCubes core UI service layer in `frontend/comfyui/ui/core/CubeLibraryApi.js`.
 */
import { isRecord } from '../types/common.js';
/**
 * Coordinate cube library api behavior for the SugarCubes UI.
 */
export class CubeLibraryApi {
    adapter;
    constructor(adapter) {
        this.adapter = adapter;
    }
    async fetchJson(url, options = {}) {
        const api = this.adapter?.getApi?.() || null;
        if (!api?.fetchApi) {
            throw new Error('ComfyUI API unavailable');
        }
        const response = await api.fetchApi(url, options);
        const decoded = await response.json().catch(() => ({}));
        const data = isRecord(decoded) ? decoded : {};
        return { response, data };
    }
    async list(options = {}) {
        return this.fetchJson('/sugarcubes/list', options);
    }
    async listTrackedRepos(options = {}) {
        return this.fetchJson('/sugarcubes/repos', options);
    }
    async getIdentityPolicy(options = {}) {
        return this.fetchJson('/sugarcubes/identity_policy', options);
    }
    async updateIdentityPolicy(payload, options = {}) {
        return this.fetchJson('/sugarcubes/identity_policy', {
            method: 'PATCH',
            body: payload,
            ...options,
        });
    }
    async listCubePacks(options = {}) {
        return this.listTrackedRepos(options);
    }
    async addTrackedRepo(payload, options = {}) {
        return this.fetchJson('/sugarcubes/repos', { method: 'POST', body: payload, ...options });
    }
    async preflightCubePack(payload, options = {}) {
        return this.fetchJson('/sugarcubes/repos/preflight', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async addCubePack(payload, options = {}) {
        return this.addTrackedRepo(payload, options);
    }
    async createAuthoringCubePack(payload, options = {}) {
        return this.fetchJson('/sugarcubes/repos/authoring', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async updateTrackedRepo(payload, options = {}) {
        return this.fetchJson('/sugarcubes/repos', { method: 'PATCH', body: payload, ...options });
    }
    async updateCubePack(payload, options = {}) {
        return this.updateTrackedRepo(payload, options);
    }
    async removeTrackedRepo({ owner, repo } = {}) {
        const params = new URLSearchParams();
        if (owner) {
            params.set('owner', String(owner));
        }
        if (repo) {
            params.set('repo', String(repo));
        }
        const query = params.toString();
        return this.fetchJson(query ? `/sugarcubes/repos?${query}` : '/sugarcubes/repos', {
            method: 'DELETE',
        });
    }
    async removeCubePack(reference = {}) {
        return this.removeTrackedRepo(reference);
    }
    async syncTrackedRepo(payload, options = {}) {
        return this.fetchJson('/sugarcubes/repos/sync', { method: 'POST', body: payload, ...options });
    }
    async updateCubePackNow(payload, options = {}) {
        return this.syncTrackedRepo(payload, options);
    }
    async syncAllTrackedRepos(options = {}) {
        return this.fetchJson('/sugarcubes/repos/sync_all', { method: 'POST', ...options });
    }
    async updateAllCubePacks(options = {}) {
        return this.syncAllTrackedRepos(options);
    }
    async checkCubePack(payload, options = {}) {
        return this.fetchJson('/sugarcubes/packs/check', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async checkAllCubePacks(payload = '{}', options = {}) {
        return this.fetchJson('/sugarcubes/packs/check_all', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async preview(cubeId) {
        const url = `/sugarcubes/preview?cube_id=${encodeURIComponent(cubeId)}`;
        return this.fetchJson(url);
    }
    async listRevisions(cubeId) {
        const url = `/sugarcubes/revisions?cube_id=${encodeURIComponent(cubeId)}`;
        return this.fetchJson(url);
    }
    async delete(reference) {
        if (typeof reference === 'string') {
            const url = `/sugarcubes?cube_id=${encodeURIComponent(reference)}`;
            return this.fetchJson(url, { method: 'DELETE' });
        }
        const params = new URLSearchParams();
        if (reference && typeof reference === 'object') {
            if (reference.cube_id) {
                params.set('cube_id', String(reference.cube_id));
            }
        }
        const query = params.toString();
        const url = query ? `/sugarcubes?${query}` : '/sugarcubes';
        return this.fetchJson(url, { method: 'DELETE' });
    }
    async load(payload, options = {}) {
        return this.fetchJson('/sugarcubes/load', { method: 'POST', body: payload, ...options });
    }
    async loadRevision(payload, options = {}) {
        return this.fetchJson('/sugarcubes/load_revision', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async rename(payload, options = {}) {
        return this.fetchJson('/sugarcubes/rename', { method: 'POST', body: payload, ...options });
    }
    async promote(payload, options = {}) {
        return this.fetchJson('/sugarcubes/promote', { method: 'POST', body: payload, ...options });
    }
    async updateMetadata(payload, options = {}) {
        return this.fetchJson('/sugarcubes/update_metadata', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async saveMany(payload, options = {}) {
        return this.fetchJson('/sugarcubes/save_many', { method: 'POST', body: payload, ...options });
    }
    async saveImplementation(payload, options = {}) {
        return this.fetchJson('/sugarcubes/save_implementation', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async saveAuthoredFlavor(payload, options = {}) {
        return this.fetchJson('/sugarcubes/save_authored_flavor', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async getLocalFlavors(cubeId, options = {}) {
        const url = `/sugarcubes/local_flavors?cube_id=${encodeURIComponent(cubeId)}`;
        return this.fetchJson(url, options);
    }
    async saveLocalFlavor(payload, options = {}) {
        return this.fetchJson('/sugarcubes/local_flavors', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async deleteLocalFlavor(payload, options = {}) {
        return this.fetchJson('/sugarcubes/local_flavors', {
            method: 'DELETE',
            body: payload,
            ...options,
        });
    }
    async selectLocalFlavor(payload, options = {}) {
        return this.fetchJson('/sugarcubes/local_flavors/select', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async migrateLocalFlavors(payload, options = {}) {
        return this.fetchJson('/sugarcubes/local_flavors/migrate', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
    async reconcileLocalFlavors(payload, options = {}) {
        return this.fetchJson('/sugarcubes/local_flavors/reconcile', {
            method: 'POST',
            body: payload,
            ...options,
        });
    }
}
