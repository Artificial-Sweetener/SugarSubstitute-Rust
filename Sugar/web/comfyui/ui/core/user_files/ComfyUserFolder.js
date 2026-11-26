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
 * One folder inside the Comfy user's own folder, reached through ComfyUI's per-user file routes
 * (`/userdata`), which apply the host's user headers and keep every path inside that folder.
 */
import { isRecord } from '../../types/common.js';
/** Name one kind of file in error messages, such as "wildcard file". */
export class ComfyUserFolder {
    fetchApi;
    folder;
    noun;
    constructor(fetchApi, 
    /** The folder inside the user's folder, such as `sugar/wildcards`. */
    folder, noun) {
        this.fetchApi = fetchApi;
        this.folder = folder;
        this.noun = noun;
    }
    /** Every file under the folder; a folder that does not exist yet holds none. */
    async list() {
        const response = await this.#request('/userdata?' +
            new URLSearchParams({ dir: this.folder, recurse: 'true', full_info: 'true' }).toString());
        if (response.status === 404)
            return [];
        if (!response.ok)
            throw new Error(`The ${this.noun} folder could not be listed (${String(response.status)}).`);
        const entries = await response.json();
        if (!Array.isArray(entries))
            throw new Error(`The ${this.noun} folder listing is invalid.`);
        return entries.flatMap((entry) => isRecord(entry) && typeof entry.path === 'string'
            ? [{ path: entry.path, stamp: `${String(entry.modified)}:${String(entry.size)}` }]
            : []);
    }
    /** One file's text; null when it does not exist. */
    async read(path) {
        const response = await this.#request(this.#location(path));
        if (response.status === 404)
            return null;
        if (!response.ok)
            throw new Error(`The ${this.noun} could not be read (${String(response.status)}).`);
        return await response.text();
    }
    /** Create a file with the given text, never replacing one that exists. */
    async create(path, text) {
        const response = await this.#post(path, text, false);
        if (response.status === 409)
            return 'exists';
        if (!response.ok)
            throw new Error(`The ${this.noun} could not be created (${String(response.status)}).`);
        return 'created';
    }
    /** Replace a file's text, creating the file when it does not exist. */
    async write(path, text) {
        const response = await this.#post(path, text, true);
        if (!response.ok)
            throw new Error(`The ${this.noun} could not be saved (${String(response.status)}).`);
    }
    /** Rename a file inside the folder, never replacing one that exists. */
    async rename(from, to) {
        const destination = encodeURIComponent(`${this.folder}/${to}`);
        const response = await this.#request(`${this.#location(from)}/move/${destination}?overwrite=false`, { method: 'POST' });
        if (response.status === 409)
            return 'exists';
        if (response.status === 404)
            return 'missing';
        if (!response.ok)
            throw new Error(`The ${this.noun} could not be renamed (${String(response.status)}).`);
        return 'renamed';
    }
    /** Delete a file; one that is already gone counts as deleted. */
    async delete(path) {
        const response = await this.#request(this.#location(path), { method: 'DELETE' });
        if (!response.ok && response.status !== 404)
            throw new Error(`The ${this.noun} could not be deleted (${String(response.status)}).`);
    }
    #post(path, text, overwrite) {
        return this.#request(`${this.#location(path)}?overwrite=${String(overwrite)}`, {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain; charset=utf-8' },
            body: text,
        });
    }
    /** Address one file; Comfy takes the whole relative path as one encoded segment. */
    #location(path) {
        return '/userdata/' + encodeURIComponent(`${this.folder}/${path}`);
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
