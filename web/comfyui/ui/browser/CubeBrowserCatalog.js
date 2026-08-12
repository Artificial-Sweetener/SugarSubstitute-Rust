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
/** Own Cube browser catalog retrieval, indexing, filtering, and grouping. */
import { normalizeSupportedModels, normalizeTargetModel } from '../core/ModelTargets.js';
import { resolveCubePackIdentity } from '../core/CubePackIdentity.js';
import { isRecord } from '../types/common.js';
import { readApiErrorMessage, readErrorMessage, } from './CubeBrowserContracts.js';
const MODEL_LIST_URLS = Object.freeze([
    new URL('../../../models.txt', import.meta.url).toString(),
    '/extensions/ComfyUI-SugarCubes/models.txt',
]);
const FALLBACK_MODEL_OPTIONS = ['Other'];
/** Coordinate the remote catalog and its local browser projection. */
export class CubeBrowserCatalog {
    options;
    cubeIdIndex = new Map();
    authorIndex = new Map();
    fetchController = null;
    refreshPromise = null;
    constructor(options) {
        this.options = options;
    }
    /** Initialize preference and model-catalog state. */
    async initialize() {
        this.options.preferences.initialize();
        this.options.store.setModelOptions(FALLBACK_MODEL_OPTIONS.slice());
        await this.loadSupportedModels().catch(() => { });
    }
    /** Return one Cube indexed by canonical ID. */
    getCubeById(cubeId) {
        const key = typeof cubeId === 'string' ? cubeId.trim() : '';
        return key ? this.cubeIdIndex.get(key) || null : null;
    }
    /** Return the canonical key for one catalog Cube. */
    getCubeKey(cube) {
        return typeof cube?.cube_id === 'string' ? cube.cube_id.trim() : '';
    }
    /** Resolve a selection key through the canonical index or legacy relative path. */
    getCubeBySelectionKey(cubeKey, entries = this.options.store.state.filtered) {
        const key = typeof cubeKey === 'string' ? cubeKey.trim() : '';
        if (!key) {
            return null;
        }
        const cached = this.cubeIdIndex.get(key);
        if (cached) {
            return cached;
        }
        return (entries.find((cube) => {
            const relativePath = typeof cube?.relative_path === 'string' ? cube.relative_path.trim() : '';
            return this.getCubeKey(cube) === key || relativePath === key;
        }) || null);
    }
    /** Coalesce and perform a catalog refresh. */
    async refresh(force) {
        if (this.refreshPromise) {
            return this.refreshPromise;
        }
        const promise = this.performRefresh(force);
        this.refreshPromise = promise;
        try {
            return await promise;
        }
        finally {
            if (this.refreshPromise === promise) {
                this.refreshPromise = null;
            }
        }
    }
    async performRefresh(force) {
        const { api, store } = this.options;
        if (!force && store.state.cubes.length) {
            this.applyFilters();
            this.options.render();
            return store.state.cubes;
        }
        this.abortPreviousRefresh();
        const controller = new AbortController();
        this.fetchController = controller;
        store.setLoading(true);
        store.setError(null);
        this.options.render();
        try {
            const { response, data } = await api.list({ signal: controller.signal });
            if (!response.ok) {
                this.reportRefreshError(readApiErrorMessage(data, response.statusText || 'Failed to load cubes'));
                return store.state.cubes;
            }
            const cubes = Array.isArray(data.cubes)
                ? data.cubes
                    .map((cube) => this.normalizeCatalogCube(cube))
                    .filter((cube) => cube !== null)
                : [];
            this.replaceCubes(cubes);
            store.setLastFetched(Date.now());
            this.options.getActions().onCubesUpdated?.(cubes);
            this.applyFilters();
            if (!store.state.selected && store.state.filtered[0]) {
                this.options.selectFirst(this.getCubeKey(store.state.filtered[0]));
            }
            else {
                this.options.render();
            }
            store.setLoading(false);
            this.options.render();
            return cubes;
        }
        catch (error) {
            if (error instanceof Error && error.name === 'AbortError') {
                return store.state.cubes;
            }
            this.reportRefreshError(readErrorMessage(error));
            return store.state.cubes;
        }
    }
    /** Normalize one raw catalog record. */
    normalizeCatalogCube(cube) {
        if (!isRecord(cube)) {
            return null;
        }
        const targetModel = normalizeTargetModel(cube.target_model);
        return {
            ...cube,
            target_model: targetModel,
            supported_models: normalizeSupportedModels(cube.supported_models, { targetModel }),
        };
    }
    /** Project current search, favorite, and author grouping state. */
    applyFilters() {
        const { store } = this.options;
        const query = (store.state.searchQuery || '').trim().toLowerCase();
        const filtered = store.state.cubes.filter((cube) => {
            if (!query) {
                return Boolean(cube && typeof cube === 'object');
            }
            const fields = [
                cube.default_alias,
                cube.display_name,
                cube.name,
                cube.description,
                Array.isArray(cube.tags) ? cube.tags.join(' ') : '',
                cube.target_model,
                Array.isArray(cube.supported_models) ? cube.supported_models.join(' ') : '',
                cube.author,
                cube.author_url,
                cube.cube_id,
                cube.version,
            ];
            return fields.filter(Boolean).join(' ').toLowerCase().includes(query);
        });
        const sorted = this.sortFavoritesFirst(filtered);
        const grouped = this.buildAuthorGroups(sorted);
        store.setFiltered(sorted);
        store.setGrouped(grouped);
        this.options.preferences.ensureAuthorGroupDefaults(grouped.map((group) => group.key));
    }
    /** Sort favorites first while preserving the established catalog order policy. */
    sortFavoritesFirst(list) {
        if (!Array.isArray(list) || !list.length) {
            return Array.isArray(list) ? list : [];
        }
        const favorites = [];
        const rest = [];
        for (const cube of list) {
            (this.options.store.state.favorites.has(this.getCubeKey(cube)) ? favorites : rest).push(cube);
        }
        const compare = (left, right) => ['target_model', 'default_alias', 'display_name', 'name', 'cube_id'].reduce((result, key) => result ||
            String(left?.[key] || '').localeCompare(String(right?.[key] || ''), undefined, {
                sensitivity: 'base',
            }), 0);
        favorites.sort(compare);
        rest.sort(compare);
        return favorites.concat(rest);
    }
    /** Return the author group containing one canonical Cube key. */
    getAuthorGroupKey(cubeKey) {
        return cubeKey ? this.authorIndex.get(cubeKey) || null : null;
    }
    /** Remove one deleted Cube from the current catalog projection. */
    removeCube(cubeKey) {
        const { store } = this.options;
        const cubes = store.state.cubes.filter((cube) => this.getCubeKey(cube) !== cubeKey);
        this.replaceCubes(cubes);
        this.applyFilters();
        if (store.state.selected === cubeKey) {
            store.setSelected(this.getCubeKey(store.state.filtered[0]) || null);
        }
    }
    replaceCubes(cubes) {
        this.options.store.setCubes(cubes);
        this.cubeIdIndex.clear();
        for (const cube of cubes) {
            const key = this.getCubeKey(cube);
            if (key) {
                this.cubeIdIndex.set(key, cube);
            }
        }
    }
    buildAuthorGroups(list) {
        const groups = new Map();
        this.authorIndex.clear();
        for (const cube of list) {
            const pack = resolveCubePackIdentity(cube);
            const entry = groups.get(pack.key) || { ...pack, cubes: [] };
            entry.cubes.push(cube);
            groups.set(pack.key, entry);
            const cubeKey = this.getCubeKey(cube);
            if (cubeKey) {
                this.authorIndex.set(cubeKey, pack.key);
            }
        }
        return Array.from(groups.values()).sort((left, right) => left.label.localeCompare(right.label));
    }
    abortPreviousRefresh() {
        try {
            this.fetchController?.abort();
        }
        catch (_error) {
            // Abort is best-effort when a host supplies a nonstandard controller.
        }
    }
    reportRefreshError(message) {
        this.options.store.setError(message);
        this.options.store.setLoading(false);
        this.options.toast?.push('error', 'Cube library unavailable', message);
        this.options.render();
    }
    async loadSupportedModels() {
        const fetchRef = this.options.adapter?.getFetch?.();
        if (!fetchRef) {
            return;
        }
        for (const url of MODEL_LIST_URLS) {
            const response = await fetchRef(url);
            if (!response?.ok) {
                continue;
            }
            const options = normalizeSupportedModels((await response.text())
                .split(/\r?\n/)
                .map((line) => line.trim())
                .filter(Boolean));
            if (options.length) {
                this.options.store.setModelOptions(options);
                this.options.render();
            }
            return;
        }
    }
}
