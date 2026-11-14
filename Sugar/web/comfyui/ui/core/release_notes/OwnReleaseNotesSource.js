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
 * Read SugarCubes' own published release notes from its backend.
 *
 * The backend reads the changelog published at the running version's release tag through its
 * bounded release client; the page never reaches the release repository itself.
 * `GET /sugarcubes/release-notes` names the product and running version, and
 * `GET /sugarcubes/release-notes/notes?since=x.y.z` answers the notes or a failure.
 */
import { isRecord } from '../../types/common.js';
import { unavailableNotes, } from './ReleaseNotesSource.js';
const VERSION_PATH = '/sugarcubes/release-notes';
const NOTES_PATH = '/sugarcubes/release-notes/notes';
const TOKEN = /^[a-z0-9.-]{1,64}$/;
function decodeIdentity(value) {
    if (!isRecord(value) || value.schema !== 1)
        return null;
    const { product, currentVersion } = value;
    if (typeof product !== 'string' || product.length === 0 || product.length > 64)
        return null;
    if (typeof currentVersion !== 'string' || !TOKEN.test(currentVersion))
        return null;
    return Object.freeze({ product, currentVersion });
}
function decodeRelease(value) {
    if (!isRecord(value))
        return null;
    const { version, notes } = value;
    if (typeof version !== 'string' || !TOKEN.test(version))
        return null;
    if (typeof notes !== 'string' || notes.length > 262144)
        return null;
    return { version, notes };
}
/** Validate one notes reply; anything unexpected reads as unavailable. */
export function decodeReleaseNotes(value) {
    if (!isRecord(value) || value.schema !== 1)
        return unavailableNotes();
    if (value.failure === 'not-published')
        return { kind: 'failed', reason: 'not-published' };
    if (value.failure !== undefined)
        return unavailableNotes();
    const { releases, omitted } = value;
    if (!Array.isArray(releases) || releases.length === 0 || releases.length > 64)
        return unavailableNotes();
    if (!Number.isSafeInteger(omitted) || Number(omitted) < 0)
        return unavailableNotes();
    const decoded = releases.map(decodeRelease);
    if (decoded.some((release) => release === null))
        return unavailableNotes();
    return {
        kind: 'notes',
        releases: decoded.filter((release) => release !== null),
        omitted: Number(omitted),
    };
}
/** SugarCubes' own release notes as a {@link ReleaseNotesSource}. */
export class OwnReleaseNotesSource {
    backend;
    report;
    #listeners = new Set();
    #identity = null;
    #retired = false;
    constructor(backend, report) {
        this.backend = backend;
        this.report = report;
    }
    /** Read the product and running version once. */
    async start() {
        let reply;
        try {
            reply = await this.backend.get(VERSION_PATH);
        }
        catch (error) {
            this.report(error);
            return;
        }
        const identity = reply.status === 200 ? decodeIdentity(reply.body) : null;
        if (this.#retired || !identity)
            return;
        this.#identity = identity;
        for (const changed of [...this.#listeners])
            changed();
    }
    /** Stop announcing; later reads settle as unavailable. */
    retire() {
        this.#retired = true;
        this.#listeners.clear();
    }
    read() {
        return this.#identity;
    }
    subscribe(changed) {
        this.#listeners.add(changed);
        return () => this.#listeners.delete(changed);
    }
    async load(since) {
        if (this.#retired || !TOKEN.test(since))
            return unavailableNotes();
        try {
            const reply = await this.backend.get(`${NOTES_PATH}?since=${encodeURIComponent(since)}`);
            return reply.status === 200 ? decodeReleaseNotes(reply.body) : unavailableNotes();
        }
        catch (error) {
            this.report(error);
            return unavailableNotes();
        }
    }
}
/** Adapt ComfyUI's `api.fetchApi` to the backend port, or return null without it. */
export function comfyReleaseNotesBackend(api) {
    const comfy = isRecord(api) ? api : null;
    const fetchApi = comfy?.fetchApi;
    if (!comfy || typeof fetchApi !== 'function')
        return null;
    return {
        async get(path) {
            const response = await fetchApi.call(comfy, path, { method: 'GET' });
            const reply = isRecord(response) ? response : null;
            const status = reply?.status;
            const json = reply?.json;
            if (typeof status !== 'number' || typeof json !== 'function')
                throw new TypeError('ComfyUI returned an unreadable reply.');
            const body = await Promise.resolve(json.call(response)).catch(() => null);
            return { status, body };
        },
    };
}
