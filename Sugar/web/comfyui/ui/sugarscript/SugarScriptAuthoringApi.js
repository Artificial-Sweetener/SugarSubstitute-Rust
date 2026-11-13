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
/** Call the additive SugarScript authoring endpoint through Comfy's API. */
import { readSugarScriptAuthoringResponse } from './SugarScriptAuthoringModels.js';
/** Own transport and response validation for source-to-native authoring. */
export class SugarScriptAuthoringApi {
    #api;
    /** Reuse SugarCubes' standard Comfy API adapter. */
    constructor(api) {
        this.#api = api;
    }
    /** Compile bounded source without mutating or queueing a workflow. */
    async compile(source) {
        const result = await this.#api.fetchJson('/sugarcubes/v2/sugarscript/compile', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ source }),
        });
        if (result.response.status !== 200 && result.response.status !== 422) {
            throw new Error(readBackendError(result.data, result.response.status));
        }
        return readSugarScriptAuthoringResponse(result.data);
    }
}
/** Keep expected backend failures useful without trusting arbitrary response data. */
function readBackendError(value, status) {
    const error = 'error' in value && typeof value.error === 'object' && value.error ? value.error : {};
    const message = 'message' in error && typeof error.message === 'string' ? error.message.trim() : '';
    return message || `SugarScript compilation failed${status ? ` (${String(status)})` : ''}.`;
}
