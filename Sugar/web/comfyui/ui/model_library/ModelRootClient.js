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
/** Validate the shared root-configuration contract at the browser transport boundary. */
import { isRecord } from '../types/common.js';
/** Reject malformed configuration responses instead of manufacturing a folder selection. */
export function decodeModelRootStatus(value) {
    if (!isRecord(value) ||
        value.schemaVersion !== 1 ||
        typeof value.defaultModelRoot !== 'string' ||
        typeof value.activeModelRoot !== 'string' ||
        !(value.configuredModelRoot === null || typeof value.configuredModelRoot === 'string') ||
        typeof value.usesDefault !== 'boolean' ||
        typeof value.restartRequired !== 'boolean' ||
        value.usesDefault !== (value.configuredModelRoot === null))
        throw new Error('The model-folder configuration response is invalid.');
    return {
        defaultModelRoot: value.defaultModelRoot,
        configuredModelRoot: value.configuredModelRoot,
        activeModelRoot: value.activeModelRoot,
        usesDefault: value.usesDefault,
        restartRequired: value.restartRequired,
    };
}
/** Keep configuration requests independent of picker mounts and current workflow selection. */
export class ModelRootClient {
    #fetch;
    /** Bind the host-addressed fetch adapter. */
    constructor(fetchApi) {
        this.#fetch = fetchApi;
    }
    /** Read active and next-launch model folder selections. */
    load() {
        return this.#request();
    }
    /** Ask the shared backend owner to persist a deliberate next-launch selection. */
    configure(path) {
        return this.#request({
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ mode: path === null ? 'default' : 'custom', path }),
        });
    }
    /** Bound transport and preserve useful server validation errors as literal UI text. */
    async #request(options) {
        const controller = new AbortController();
        const timeout = window.setTimeout(() => controller.abort(), 20000);
        try {
            const response = await this.#fetch('/sugarcubes/v1/models/root', {
                ...options,
                signal: controller.signal,
            });
            const payload = await response.json();
            if (!response.ok)
                throw new Error(isRecord(payload) && typeof payload.error === 'string'
                    ? payload.error
                    : 'Model-folder configuration is unavailable.');
            return decodeModelRootStatus(payload);
        }
        catch (error) {
            if (controller.signal.aborted)
                throw new Error('The model-folder request timed out.', { cause: error });
            throw error;
        }
        finally {
            window.clearTimeout(timeout);
        }
    }
}
