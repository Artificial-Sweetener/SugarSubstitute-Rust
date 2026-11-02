import { CivitaiCredentialClient } from './CivitaiCredentialClient.js';
import { decodeModelCatalog } from './ModelCatalog.js';
import { isRecord } from '../types/common.js';
import { modelApiError } from './ModelApiTransport.js';
import { ModelJobClient } from './ModelJobClient.js';
import { ModelCatalogSubscription } from './ModelCatalogSubscription.js';
import { decodeModelReleases } from './ModelReleases.js';
/** Share one validated snapshot across every graph and masonry model field. */
export class ModelCatalogClient {
    credentials;
    #fetch;
    #jobs;
    #installed;
    #changes;
    #pending = null;
    #snapshot = null;
    #prioritized = new Set();
    /** Bind Comfy's backend-addressing adapter rather than assuming a localhost origin. */
    constructor(fetchApi, installed = null) {
        this.#fetch = fetchApi;
        this.credentials = new CivitaiCredentialClient(fetchApi);
        this.#jobs = new ModelJobClient(fetchApi);
        this.#installed = installed;
        this.#changes = new ModelCatalogSubscription(fetchApi, (force) => this.load(force));
    }
    /** Expose the prepared catalog without starting IO from consumer interaction paths. */
    current() {
        return this.#snapshot;
    }
    /** Persist a user classification independently of provider facts and refresh prepared metadata. */
    async classifyContent(modelId, nsfw, signal) {
        const response = await this.#fetch(`/sugarcubes/v1/models/${encodeURIComponent(modelId)}/content`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ nsfw }),
            signal,
        });
        if (!response.ok)
            throw await modelApiError(response, 'The content classification could not be saved.');
        await this.load(true);
    }
    /** Read the shared downloader pattern through the backend preference owner. */
    async downloadPattern() {
        const response = await this.#fetch('/sugarcubes/v1/models/download-preferences');
        if (!response.ok)
            throw await modelApiError(response, 'Download organization is unavailable.');
        const payload = await response.json();
        if (!isRecord(payload) || payload.schemaVersion !== 1 || typeof payload.pattern !== 'string')
            throw new Error('Invalid download organization settings.');
        return payload.pattern;
    }
    /** Save a relative organizing pattern without changing existing model paths. */
    async saveDownloadPattern(pattern) {
        const response = await this.#fetch('/sugarcubes/v1/models/download-preferences', {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ pattern }),
        });
        if (!response.ok)
            throw await modelApiError(response, 'The download organization could not be saved.');
    }
    /** Reuse a completed snapshot or await the same in-flight HTTP request. */
    load(refresh = false) {
        if (this.#pending)
            return this.#pending;
        if (this.#snapshot && !refresh)
            return Promise.resolve(this.#snapshot);
        this.#pending = this.#read().finally(() => {
            this.#pending = null;
        });
        return this.#pending;
    }
    /** Request one explicit background stat scan through the catalog owner. */
    async refresh() {
        const response = await this.#fetch('/sugarcubes/v1/models/refresh', { method: 'POST' });
        if (!response.ok)
            throw new Error(`Model library refresh failed (${String(response.status)}).`);
        this.#snapshot = null;
    }
    /** Give visible catalog choices priority without changing values or creating client-owned jobs. */
    async prioritize(items) {
        const identities = [...new Set(items.map((item) => item.id))]
            .filter((id) => /^[a-f0-9]{64}$/.test(id) && !this.#prioritized.has(id))
            .slice(0, 128);
        if (!identities.length)
            return;
        for (const id of identities)
            this.#prioritized.add(id);
        try {
            const response = await this.#fetch('/sugarcubes/v1/models/metadata/prioritize', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ modelIds: identities }),
            });
            if (!response.ok)
                throw new Error('Background model presentation could not be prioritized.');
        }
        catch (error) {
            for (const id of identities)
                this.#prioritized.delete(id);
            throw error;
        }
    }
    /** Observe the server-owned job and cancel it when its requesting presentation closes. */
    async refreshMetadata(modelId, progress, signal) {
        const response = await this.#fetch(`/sugarcubes/v1/models/${encodeURIComponent(modelId)}/metadata/refresh`, { method: 'POST', signal });
        if (!response.ok)
            throw new Error('Model metadata refresh could not be started.');
        const result = await this.#jobs.observe(response, {
            queued: 'Waiting to refresh metadata…',
            running: 'Reading fingerprint and refreshing metadata…',
        }, progress, signal);
        progress(typeof result.warning === 'string'
            ? result.warning
            : result.status === 'not_found'
                ? 'No provider match was found.'
                : 'Metadata refreshed.');
        await this.load(true);
    }
    /** Upload only selected image bytes; model paths and remote addresses remain server-owned. */
    async replaceThumbnail(modelId, image, signal) {
        if (!image.size || image.size > 20 * 1024 * 1024)
            throw new Error('Choose an image smaller than 20 MiB.');
        const response = await this.#fetch(`/sugarcubes/v1/models/${encodeURIComponent(modelId)}/thumbnail`, { method: 'PUT', body: image, signal });
        if (!response.ok)
            throw new Error('The model preview could not be saved.');
        await this.load(true);
    }
    /** Read provider history without changing the field or initiating a model fingerprint. */
    async releases(modelId, signal) {
        const response = await this.#fetch(`/sugarcubes/v1/models/${encodeURIComponent(modelId)}/versions`, { signal });
        if (!response.ok)
            throw await modelApiError(response, 'Model versions are unavailable. Try again later.');
        return decodeModelReleases(await response.json());
    }
    /** Acquire only server-offered artifact identities and refresh host choices after publication. */
    async acquire(modelId, versionId, fileId, progress, signal, transfer) {
        const response = await this.#fetch(`/sugarcubes/v1/models/${encodeURIComponent(modelId)}/acquire`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ versionId, fileId }),
            signal,
        });
        if (!response.ok)
            throw await modelApiError(response, 'The model download could not be started.');
        const result = await this.#jobs.observe(response, { queued: 'Waiting to download…', running: 'Downloading and verifying model…' }, progress, signal, transfer);
        if (result.status !== 'installed' || typeof result.fileName !== 'string')
            throw new Error('Invalid model download result.');
        await this.#installed?.();
        await this.load(true);
        return result.fileName;
    }
    /** Follow committed catalog generations through the page's one shared change subscription. */
    watch(listener, reportError) {
        return this.#changes.watch(listener, reportError);
    }
    /** Validate unknown JSON before publishing it to any picker consumer. */
    async #read() {
        const response = await this.#fetch('/sugarcubes/v1/models');
        if (!response.ok)
            throw new Error(`Model catalog is unavailable (${String(response.status)}).`);
        const payload = await response.json();
        this.#snapshot = decodeModelCatalog(payload);
        return this.#snapshot;
    }
}
