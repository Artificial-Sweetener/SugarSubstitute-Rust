import { isRecord } from '../types/common.js';
import { decodeOutputSavingSnapshot, } from './OutputSavingPreferences.js';
/** Expose bounded commands independently of settings rendering and the canvas. */
export class OutputSavingClient {
    api;
    /** Bind the established host-addressed JSON transport. */
    constructor(api) {
        this.api = api;
    }
    /** Read validated saving policy without changing user preferences. */
    load(signal) {
        return this.#request('settings', undefined, signal);
    }
    /** Commit a complete deliberate settings edit before subsequent runs capture it. */
    save(preferences) {
        return this.#request('settings', preferences);
    }
    /** Render a proposed path without saving the draft or allocating output files. */
    preview(preferences, signal) {
        return this.#request('preview', preferences, signal);
    }
    /** Import explicit source bytes without reading or modifying the user's Substitute settings file. */
    importSubstitute(value) {
        return this.#request('import', value);
    }
    /** Bound network lifetime and preserve actionable literal server errors for SugarCubes feedback. */
    async #request(action, value, signal) {
        const controller = new AbortController();
        const abort = () => controller.abort();
        if (signal?.aborted)
            abort();
        signal?.addEventListener('abort', abort, { once: true });
        const timeout = setTimeout(abort, 20000);
        try {
            const options = { signal: controller.signal };
            if (value !== undefined) {
                options.method = 'POST';
                options.headers = { 'Content-Type': 'application/json' };
                options.body = JSON.stringify(value);
            }
            const { response, data } = await this.api.fetchJson('/sugarcubes/output-saving/' + action, options);
            if (response.ok === false || data.success !== true) {
                const error = isRecord(data.error) ? data.error.message : undefined;
                throw new Error(typeof error === 'string' ? error : 'Output saving settings are unavailable.');
            }
            return decodeOutputSavingSnapshot(data.data);
        }
        finally {
            clearTimeout(timeout);
            signal?.removeEventListener('abort', abort);
        }
    }
}
