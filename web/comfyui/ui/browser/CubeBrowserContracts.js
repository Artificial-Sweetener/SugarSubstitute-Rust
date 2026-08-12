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
/** Define the host and application boundaries used by the Cube browser owners. */
import { isRecord } from '../types/common.js';
const unavailableApiCall = async () => {
    throw new Error('Cube library API unavailable');
};
const UNAVAILABLE_BROWSER_API = {
    list: unavailableApiCall,
    listRevisions: unavailableApiCall,
    delete: unavailableApiCall,
    load: unavailableApiCall,
    loadRevision: unavailableApiCall,
    rename: unavailableApiCall,
    updateMetadata: unavailableApiCall,
};
/** Bind every optional API operation to one complete browser API boundary. */
export function resolveBrowserApi(api) {
    return {
        list: api?.list?.bind(api) ?? UNAVAILABLE_BROWSER_API.list,
        listRevisions: api?.listRevisions?.bind(api) ?? UNAVAILABLE_BROWSER_API.listRevisions,
        delete: api?.delete?.bind(api) ?? UNAVAILABLE_BROWSER_API.delete,
        load: api?.load?.bind(api) ?? UNAVAILABLE_BROWSER_API.load,
        loadRevision: api?.loadRevision?.bind(api) ?? UNAVAILABLE_BROWSER_API.loadRevision,
        rename: api?.rename?.bind(api) ?? UNAVAILABLE_BROWSER_API.rename,
        updateMetadata: api?.updateMetadata?.bind(api) ?? UNAVAILABLE_BROWSER_API.updateMetadata,
    };
}
/** Return whether an API payload contains a structured error. */
export function hasApiError(data) {
    return isRecord(data.error);
}
/** Read the most specific browser API error message available. */
export function readApiErrorMessage(data, fallback) {
    const error = isRecord(data.error) ? data.error : null;
    return (typeof error?.message === 'string' && error.message.trim()) || fallback;
}
/** Normalize an unknown thrown value for browser feedback. */
export function readErrorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}
