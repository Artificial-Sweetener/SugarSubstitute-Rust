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
/** Own bounded HTTP requests and validated ephemeral spatial-probe handles. */
import { isRecord } from '../../types/common.js';
const route = '/sugarcubes/v2/executions/spatial';
/** Keep backend protocol and cleanup mechanics out of field menus and command policy. */
export class ComfySpatialProbeClient {
    fetchApi;
    report;
    /** Retain the host's backend addressing and actionable cleanup diagnostics. */
    constructor(fetchApi, report) {
        this.fetchApi = fetchApi;
        this.report = report;
    }
    /** Admit one frozen compiled prompt without queuing any work until its output is explicitly chosen. */
    async prepare(prompt, width, height, cancellation) {
        if (cancellation?.cancelled())
            throw new Error('Scale discovery cancelled.');
        const response = await this.#request('/prepare', { prompt, width_targets: width, height_targets: height }, cancellation);
        const value = await response.json();
        if (!isRecord(value) ||
            typeof value.token !== 'string' ||
            !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value.token) ||
            !Array.isArray(value.outputs) ||
            value.outputs.length > 64)
            throw new Error('The backend returned an invalid spatial probe.');
        const token = value.token;
        const release = () => {
            void this.#request(`/${token}`, null).catch(this.report);
        };
        let outputs;
        try {
            outputs = value.outputs.map((item) => {
                if (!isRecord(item) ||
                    typeof item.id !== 'string' ||
                    !item.id ||
                    item.id.length > 512 ||
                    typeof item.label !== 'string' ||
                    item.label.length > 256)
                    throw new Error('The backend returned an invalid probe output.');
                return { id: item.id, label: item.label };
            });
            if (cancellation?.cancelled())
                throw new Error('Scale discovery cancelled.');
        }
        catch (error) {
            release();
            throw error;
        }
        let disposed = false;
        return {
            outputs,
            discover: async (output, stop) => {
                if (disposed)
                    throw new Error('The captured probe is unavailable.');
                if (!outputs.some((item) => item.id === output))
                    throw new Error('Choose an available probe output.');
                const response = await this.#request(`/${token}/measure`, { output }, stop);
                const result = await response.json();
                if (!isRecord(result) ||
                    typeof result.scale !== 'number' ||
                    !Number.isFinite(result.scale) ||
                    result.scale <= 0 ||
                    result.scale > 1000)
                    throw new Error('The backend did not return a valid measured scale.');
                return result.scale;
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                release();
            },
        };
    }
    /** Bound each request and retire caller cancellation before publishing any result. */
    async #request(path, body, cancellation) {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), path.endsWith('/measure') ? 120000 : 15000);
        const stop = cancellation?.subscribe(() => controller.abort());
        try {
            const response = await this.fetchApi(route + path, {
                method: body === null ? 'DELETE' : 'POST',
                headers: { 'Content-Type': 'application/json' },
                ...(body === null ? {} : { body: JSON.stringify(body) }),
                signal: controller.signal,
            });
            if (!response.ok) {
                const value = await response.json();
                throw new Error(isRecord(value) && typeof value.error === 'string'
                    ? value.error
                    : isRecord(value) && isRecord(value.error) && typeof value.error.message === 'string'
                        ? value.error.message
                        : 'Spatial scale discovery failed.');
            }
            return response;
        }
        finally {
            stop?.();
            clearTimeout(timer);
        }
    }
}
