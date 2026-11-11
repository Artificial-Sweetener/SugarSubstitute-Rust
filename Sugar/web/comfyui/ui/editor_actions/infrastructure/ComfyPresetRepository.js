import { decodePresetDocument, encodePresetDocument } from '../domain/PresetCodec.js';
const path = '/userdata/SugarCubes%2Fpresets.json';
/** Keep libraries portable across browsers by storing them with the current Comfy user. */
export class ComfyPresetRepository {
    #fetch;
    /** Bind the host's configured backend addressing and user headers. */
    constructor(fetchApi) {
        this.#fetch = fetchApi;
    }
    /** Treat only a missing file as a fresh library; errors must never become an empty save. */
    async load() {
        const response = await this.#request();
        if (response.status === 404)
            return [];
        if (!response.ok)
            throw new Error(`Preset library could not be read (${String(response.status)}).`);
        return decodePresetDocument(await response.json());
    }
    /** Persist the validated common schema through Comfy's same-directory atomic replacement. */
    async save(presets) {
        const response = await this.#request({
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(encodePresetDocument(presets)),
        });
        if (!response.ok)
            throw new Error(`Preset library could not be saved (${String(response.status)}).`);
    }
    /** Bound network IO and release the timeout after the host request settles. */
    async #request(options = {}) {
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 10_000);
        try {
            return await this.#fetch(path, { ...options, signal: controller.signal });
        }
        finally {
            clearTimeout(timeout);
        }
    }
}
