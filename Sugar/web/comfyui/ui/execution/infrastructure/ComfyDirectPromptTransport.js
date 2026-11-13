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
/** Route Cube submissions at Comfy's transport boundary while preserving queue observers. */
import { isRecord } from '../../types/common.js';
const requestField = '__sugarcubesQueueRequest';
/** Correlate each direct request explicitly without changing the user's graph or host queue. */
export class ComfyDirectPromptTransport {
    #api;
    #nextId;
    #response;
    #pending = new Map();
    #installation = null;
    /** Validate the host transport and bind response construction at the browser boundary. */
    constructor(options) {
        if (!isTransportApi(options.api))
            throw new TypeError('Comfy prompt queue transport is unavailable.');
        this.#api = options.api;
        this.#nextId = options.nextId ?? (() => crypto.randomUUID());
        this.#response = options.response ?? promptResponse;
    }
    /** Traverse native queue observers and substitute exactly one correlated POST operation. */
    async queue(payload, execute, submit) {
        if (!isRecord(payload) || !isRecord(payload.workflow))
            throw new TypeError('Direct execution requires a serialized Comfy workflow.');
        const id = this.#nextId();
        if (this.#pending.has(id))
            throw new Error('Duplicate Cube queue request identity.');
        const submission = { execute, claimed: false };
        this.#pending.set(id, submission);
        try {
            this.#install();
            const result = await submit({
                ...payload,
                workflow: { ...payload.workflow, [requestField]: id },
            });
            if (!submission.claimed)
                throw new Error('The Cube submission did not reach the Comfy prompt transport.');
            return result;
        }
        finally {
            this.#pending.delete(id);
            this.#release();
        }
    }
    /** Install a pass-through adapter only while direct submissions need correlation. */
    #install() {
        if (this.#installation)
            return;
        const original = this.#api.fetchApi;
        const wrapper = async (path, options) => {
            const id = promptRequestId(path, options);
            if (id === null)
                return await original.call(this.#api, path, options);
            const submission = this.#pending.get(id);
            if (!submission || submission.claimed)
                throw new Error('The Cube prompt transport received an expired or duplicate request.');
            submission.claimed = true;
            return this.#response(await submission.execute());
        };
        this.#installation = { original, wrapper };
        this.#api.fetchApi = wrapper;
    }
    /** Restore only our own transport once every concurrent submission has settled. */
    #release() {
        if (this.#pending.size || !this.#installation)
            return;
        if (this.#api.fetchApi === this.#installation.wrapper)
            this.#api.fetchApi = this.#installation.original;
        this.#installation = null;
    }
}
/** Narrow the external host before any internal collaboration uses it. */
function isTransportApi(api) {
    return isRecord(api) && typeof api.fetchApi === 'function';
}
/** Read our ephemeral correlation field from Comfy's serialized POST body. */
function promptRequestId(path, options) {
    if (path !== '/prompt' ||
        options?.method?.toUpperCase() !== 'POST' ||
        typeof options.body !== 'string')
        return null;
    let body;
    try {
        body = JSON.parse(options.body);
    }
    catch {
        // Malformed native requests retain Comfy's own transport error handling.
        return null;
    }
    if (!isRecord(body) || !isRecord(body.extra_data) || !isRecord(body.extra_data.extra_pnginfo))
        return null;
    const workflow = body.extra_data.extra_pnginfo.workflow;
    if (!isRecord(workflow))
        return null;
    const id = workflow[requestField];
    return typeof id === 'string' ? id : null;
}
/** Return the full browser response contract expected by native queue wrappers. */
function promptResponse(value) {
    return new Response(JSON.stringify(value), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
    });
}
