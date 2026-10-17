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
/** Read installed runtime metadata through Comfy's authenticated same-origin API. */
import { api } from '/scripts/api.js';
import { isRecord } from '../types/common.js';
import { sugarCubesProject, } from './AboutInformation.js';
/** Isolate browser transport from the About page and future native metadata providers. */
export class ComfyAboutInformation {
    request;
    resource;
    noticesUrl = new URL('../../../third_party/NOTICE.md', import.meta.url).href;
    /** Accept transport injection for deterministic metadata and failure tests. */
    constructor(request = (path, options) => api.fetchApi(path, options), resource = (path, options) => fetch(path, options)) {
        this.request = request;
        this.resource = resource;
    }
    /** Combine live product versions with pinned presentation libraries and expose partial availability honestly. */
    async load() {
        const results = await Promise.allSettled([
            this.#json('/sugarcubes/status'),
            this.#json('/system_stats'),
        ]);
        const sugar = results[0];
        const comfy = results[1];
        const status = sugar?.status === 'fulfilled' && isRecord(sugar.value) ? sugar.value : {};
        const stats = comfy?.status === 'fulfilled' && isRecord(comfy.value) ? comfy.value : {};
        const system = isRecord(stats.system) ? stats.system : {};
        return {
            components: [
                { ...sugarCubesProject, version: this.#text(status.sugarCubesVersion) },
                {
                    name: 'ComfyUI',
                    description: 'Workflow engine and node editor',
                    author: 'Comfy Org and contributors',
                    version: this.#text(system.comfyui_version),
                    url: 'https://github.com/Comfy-Org/ComfyUI',
                },
                {
                    name: 'Microsoft Fluent UI',
                    description: 'Fluent web components',
                    author: 'Microsoft and contributors',
                    version: '3.1.3',
                    url: 'https://github.com/microsoft/fluentui',
                },
                {
                    name: 'Adobe Spectrum',
                    description: 'Spectrum Web Components with Spectrum 2 foundations',
                    author: 'Adobe and contributors',
                    version: '1.12.5',
                    url: 'https://github.com/adobe/spectrum-web-components',
                },
            ],
            warnings: results.flatMap((result, index) => result.status === 'rejected'
                ? [
                    `${index === 0 ? 'SugarCubes' : 'ComfyUI'} version information is unavailable: ${String(result.reason instanceof Error ? result.reason.message : result.reason)}`,
                ]
                : []),
        };
    }
    /** Load the exact license shipped with the extension, including offline installations. */
    async license() {
        const response = await this.resource(new URL('../../../legal/AGPL-3.0.txt', import.meta.url).pathname, { signal: AbortSignal.timeout(5000) });
        if (!response.ok)
            throw new Error(`License request failed (${response.status}).`);
        return response.text();
    }
    /** Bound metadata requests and reject protocol errors before decoding untrusted JSON. */
    async #json(path) {
        const response = await this.request(path, { signal: AbortSignal.timeout(5000) });
        if (!response.ok)
            throw new Error(`Metadata request failed (${response.status}).`);
        return response.json();
    }
    /** Avoid presenting missing or malformed version data as a successful runtime detection. */
    #text(value) {
        return typeof value === 'string' && value.trim() ? value : 'Unavailable';
    }
}
