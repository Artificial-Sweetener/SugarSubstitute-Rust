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
import type { UnknownRecord } from '../types/common.js';
import type { ComfyHostApi } from '../types/graph.js';

interface CubeApiAdapter {
  getApi?(): ComfyHostApi | null;
}

interface RepoReference {
  owner?: unknown;
  repo?: unknown;
}

export interface ApiResultResponse {
  ok?: boolean;
  status?: number;
  statusText?: string;
}

export interface ApiJsonResult {
  response: ApiResultResponse;
  data: UnknownRecord;
}

/**
 * Coordinate cube library api behavior for the SugarCubes UI.
 */
export class CubeLibraryApi {
  private readonly adapter: CubeApiAdapter;

  constructor(adapter: CubeApiAdapter) {
    this.adapter = adapter;
  }

  async fetchJson(url: string, options: RequestInit = {}): Promise<ApiJsonResult> {
    const api = this.adapter?.getApi?.() || null;
    if (!api?.fetchApi) {
      throw new Error('ComfyUI API unavailable');
    }
    const response = await api.fetchApi(url, options);
    const decoded: unknown = await response.json().catch(() => ({}));
    const data = isRecord(decoded) ? decoded : {};
    return { response, data };
  }

  async list(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/list', options);
  }

  async listTrackedRepos(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos', options);
  }

  async getIdentityPolicy(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/identity_policy', options);
  }

  async updateIdentityPolicy(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/identity_policy', {
      method: 'PATCH',
      body: payload,
      ...options,
    });
  }

  async listCubePacks(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.listTrackedRepos(options);
  }

  async addTrackedRepo(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos', { method: 'POST', body: payload, ...options });
  }

  async preflightCubePack(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos/preflight', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async addCubePack(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.addTrackedRepo(payload, options);
  }

  async createAuthoringCubePack(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos/authoring', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async updateTrackedRepo(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos', { method: 'PATCH', body: payload, ...options });
  }

  async updateCubePack(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.updateTrackedRepo(payload, options);
  }

  async removeTrackedRepo({ owner, repo }: RepoReference = {}): Promise<ApiJsonResult> {
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

  async removeCubePack(reference: RepoReference = {}): Promise<ApiJsonResult> {
    return this.removeTrackedRepo(reference);
  }

  async syncTrackedRepo(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos/sync', { method: 'POST', body: payload, ...options });
  }

  async updateCubePackNow(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.syncTrackedRepo(payload, options);
  }

  async syncAllTrackedRepos(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/repos/sync_all', { method: 'POST', ...options });
  }

  async updateAllCubePacks(options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.syncAllTrackedRepos(options);
  }

  async checkCubePack(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/packs/check', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async checkAllCubePacks(
    payload: BodyInit | null = '{}',
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/packs/check_all', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async preview(cubeId: string): Promise<ApiJsonResult> {
    const url = `/sugarcubes/preview?cube_id=${encodeURIComponent(cubeId)}`;
    return this.fetchJson(url);
  }

  async listRevisions(cubeId: string): Promise<ApiJsonResult> {
    const url = `/sugarcubes/revisions?cube_id=${encodeURIComponent(cubeId)}`;
    return this.fetchJson(url);
  }

  async delete(reference?: string | UnknownRecord | null): Promise<ApiJsonResult> {
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

  async load(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/load', { method: 'POST', body: payload, ...options });
  }

  async loadRevision(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/load_revision', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async rename(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/rename', { method: 'POST', body: payload, ...options });
  }

  async promote(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/promote', { method: 'POST', body: payload, ...options });
  }

  async updateMetadata(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/update_metadata', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async saveMany(payload: BodyInit | null, options: RequestInit = {}): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/save_many', { method: 'POST', body: payload, ...options });
  }

  async saveImplementation(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/save_implementation', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async saveAuthoredFlavor(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/save_authored_flavor', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async getLocalFlavors(cubeId: string, options: RequestInit = {}): Promise<ApiJsonResult> {
    const url = `/sugarcubes/local_flavors?cube_id=${encodeURIComponent(cubeId)}`;
    return this.fetchJson(url, options);
  }

  async saveLocalFlavor(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/local_flavors', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async deleteLocalFlavor(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/local_flavors', {
      method: 'DELETE',
      body: payload,
      ...options,
    });
  }

  async selectLocalFlavor(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/local_flavors/select', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async migrateLocalFlavors(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/local_flavors/migrate', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }

  async reconcileLocalFlavors(
    payload: BodyInit | null,
    options: RequestInit = {},
  ): Promise<ApiJsonResult> {
    return this.fetchJson('/sugarcubes/local_flavors/reconcile', {
      method: 'POST',
      body: payload,
      ...options,
    });
  }
}
