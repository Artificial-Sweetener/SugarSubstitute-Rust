import { modelApiError } from './ModelApiTransport.js';
import { decodeModelUpdates } from './ModelUpdates.js';
/** Keep update decisions separate from catalog values and native field callbacks. */
export class ModelUpdateClient {
    #fetch;
    #listeners = new Set();
    #snapshot = { enabled: false, proposals: [] };
    #checking = null;
    #selections = new Map();
    /** Bind the host HTTP adapter without owning focus or settings widgets. */
    constructor(fetch) {
        this.#fetch = fetch;
    }
    /** Return the latest validated server snapshot. */
    read() {
        return this.#snapshot;
    }
    /** Subscribe projected banners and tiles without per-field provider requests. */
    subscribe(listener) {
        this.#listeners.add(listener);
        return () => {
            this.#listeners.delete(listener);
        };
    }
    /** Load consent and existing proposals without starting provider traffic. */
    load() {
        return this.#request('/sugarcubes/v1/models/updates');
    }
    /** Coalesce focus events, allowing the authoritative server to enforce opt-in and relevance. */
    check() {
        this.#checking ??= this.#request('/sugarcubes/v1/models/updates/check', {
            method: 'POST',
        }).finally(() => {
            this.#checking = null;
        });
        return this.#checking;
    }
    /** Share selection requests while the backend owns persistent daily provider freshness. */
    checkSelection(modelId) {
        if (!/^[a-f0-9]{64}$/.test(modelId))
            return Promise.reject(new Error('Invalid model update identity.'));
        let pending = this.#selections.get(modelId);
        if (!pending) {
            pending = this.#request(`/sugarcubes/v1/models/${modelId}/updates/check`, {
                method: 'POST',
            }).finally(() => {
                this.#selections.delete(modelId);
            });
            this.#selections.set(modelId, pending);
        }
        return pending;
    }
    /** Persist deliberate consent through its single server owner. */
    setEnabled(enabled) {
        return this.#request('/sugarcubes/v1/models/updates', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ enabled }),
        });
    }
    /** Dismiss only the version currently offered for one catalog identity. */
    dismiss(modelId) {
        return this.#decision(modelId, 'dismiss');
    }
    /** Persist provider-page opt-out through a verified server-side proposal. */
    disableModel(modelId) {
        return this.#decision(modelId, 'disable');
    }
    /** Send an exact local identity instead of trusting provider IDs from presentation. */
    #decision(modelId, decision) {
        if (!/^[a-f0-9]{64}$/.test(modelId))
            return Promise.reject(new Error('Invalid model update identity.'));
        return this.#request(`/sugarcubes/v1/models/${modelId}/updates/${decision}`, {
            method: 'POST',
        });
    }
    /** Publish state only after HTTP and payload validation succeed. */
    async #request(path, options) {
        const response = await this.#fetch(path, options);
        if (!response.ok)
            throw await modelApiError(response, 'Model updates are unavailable.');
        const snapshot = decodeModelUpdates(await response.json());
        this.#snapshot = snapshot;
        for (const listener of this.#listeners)
            listener(snapshot);
    }
}
