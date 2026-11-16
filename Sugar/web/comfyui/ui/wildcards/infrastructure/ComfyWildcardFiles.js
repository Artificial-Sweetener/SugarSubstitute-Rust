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
/** Reach the user's wildcard folder through Comfy's per-user file storage. */
import { isRecord } from '../../types/common.js';
import { wildcardFile } from '../domain/WildcardSource.js';
/** The wildcard folder inside the Comfy user's own folder: `user/<user>/sugar/wildcards`. */
export const WILDCARD_FOLDER = 'sugar/wildcards';
/** List, read, create and rename wildcard files with the host's addressing and user headers. */
export class ComfyWildcardFiles {
    fetchApi;
    /** Bind the host's configured backend fetch. */
    constructor(fetchApi) {
        this.fetchApi = fetchApi;
    }
    /** List every wildcard file; a folder that does not exist yet holds none. */
    async list() {
        const response = await this.#request('/userdata?' +
            new URLSearchParams({
                dir: WILDCARD_FOLDER,
                recurse: 'true',
                full_info: 'true',
            }).toString());
        if (response.status === 404)
            return [];
        if (!response.ok)
            throw new Error(`The wildcard folder could not be listed (${String(response.status)}).`);
        const entries = await response.json();
        if (!Array.isArray(entries))
            throw new Error('The wildcard folder listing is invalid.');
        return entries.flatMap((entry) => {
            if (!isRecord(entry) || typeof entry.path !== 'string')
                return [];
            const file = wildcardFile(entry.path, `${String(entry.modified)}:${String(entry.size)}`);
            return file ? [file] : [];
        });
    }
    /** Read one file's text; null when it no longer exists. */
    async read(path) {
        const response = await this.#request(location(path));
        if (response.status === 404)
            return null;
        if (!response.ok)
            throw new Error(`The wildcard file could not be read (${String(response.status)}).`);
        return await response.text();
    }
    /** Create a file with the given text, never replacing one that exists. */
    async create(path, text) {
        const response = await this.#request(location(path) + '?overwrite=false', {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain; charset=utf-8' },
            body: text,
        });
        if (response.status === 409)
            return 'exists';
        if (!response.ok)
            throw new Error(`The wildcard file could not be created (${String(response.status)}).`);
        return 'created';
    }
    /** Rename a file inside the folder, never replacing one that exists. */
    async rename(from, to) {
        const destination = encodeURIComponent(`${WILDCARD_FOLDER}/${to}`);
        const response = await this.#request(`${location(from)}/move/${destination}?overwrite=false`, {
            method: 'POST',
        });
        if (response.status === 409)
            return 'exists';
        if (response.status === 404)
            return 'missing';
        if (!response.ok)
            throw new Error(`The wildcard file could not be renamed (${String(response.status)}).`);
        return 'renamed';
    }
    /** Show the folder in the file manager of the machine ComfyUI runs on, creating it if needed. */
    async openFolder() {
        const response = await this.#request('/sugarcubes/wildcards/open_folder', { method: 'POST' });
        if (response.ok)
            return;
        const body = await response.json().catch(() => null);
        const error = isRecord(body) && isRecord(body.error) ? body.error.message : null;
        throw new Error(typeof error === 'string' && error ? error : 'The wildcard folder could not be opened.');
    }
    /** Bound network IO and release the timeout after the host request settles. */
    async #request(path, options = {}) {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 30_000);
        try {
            return await this.fetchApi(path, { ...options, signal: controller.signal });
        }
        finally {
            clearTimeout(timeout);
        }
    }
}
/** Address one file of the folder; Comfy takes the whole relative path as one encoded segment. */
function location(path) {
    return '/userdata/' + encodeURIComponent(`${WILDCARD_FOLDER}/${path}`);
}
