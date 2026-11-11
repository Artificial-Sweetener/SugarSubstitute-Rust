import { normalizeSupportedModels, normalizeTargetModel } from './ModelTargets.js';
import { isRecord } from '../types/common.js';
/** Publish complete metadata snapshots while retaining the last successful catalog. */
export class CubeLibraryCatalog {
    #options;
    #entries = [];
    #index = new Map();
    #loaded = false;
    #refreshing = null;
    #controller = null;
    /** Bind the metadata transport and its surviving consumers. */
    constructor(options) {
        this.#options = options;
    }
    /** Return an isolated list without exposing the catalog's array ownership. */
    getCubes() {
        return this.#entries.slice();
    }
    /** Resolve metadata by the stable Cube identity. */
    getCubeById(cubeId) {
        return this.#index.get(cubeId.trim()) ?? null;
    }
    /** Coalesce metadata requests and reuse successfully loaded snapshots. */
    refresh(options = {}) {
        if (this.#refreshing)
            return this.#refreshing;
        if (this.#loaded && !options.force)
            return Promise.resolve(this.getCubes());
        const operation = this.#load().finally(() => {
            if (this.#refreshing === operation)
                this.#refreshing = null;
        });
        this.#refreshing = operation;
        return operation;
    }
    /** Cancel pending transport when the owning host is disposed. */
    dispose() {
        this.#controller?.abort();
    }
    /** Validate and publish one network result atomically. */
    async #load() {
        const controller = new AbortController();
        this.#controller = controller;
        const timeout = setTimeout(() => controller.abort(), 15000);
        try {
            const { response, data } = await this.#options.api.list({ signal: controller.signal });
            if (!response.ok || data.error || !Array.isArray(data.cubes)) {
                throw new Error('Cube library metadata response is unavailable or incompatible.');
            }
            const entries = data.cubes
                .map(readCubeLibraryEntry)
                .filter((entry) => entry !== null);
            const index = new Map(entries.map((entry) => [entry.cube_id, entry]));
            if (index.size !== entries.length)
                throw new Error('Cube library metadata contains duplicate identities.');
            if (controller.signal.aborted)
                return this.getCubes();
            this.#entries = entries;
            this.#index = index;
            this.#loaded = true;
            this.#options.onUpdated(this.getCubes());
        }
        catch (error) {
            this.#options.logger.warn('SugarCubes library metadata refresh failed; retaining the previous catalog.', { error });
        }
        finally {
            clearTimeout(timeout);
            if (this.#controller === controller)
                this.#controller = null;
        }
        return this.getCubes();
    }
}
/** Narrow dynamic metadata while preserving extension fields and fork provenance. */
function readCubeLibraryEntry(value) {
    if (!isRecord(value) || typeof value.cube_id !== 'string' || !value.cube_id.trim())
        return null;
    const targetModel = normalizeTargetModel(value.target_model);
    const entry = {
        ...value,
        cube_id: value.cube_id.trim(),
        target_model: targetModel,
        supported_models: normalizeSupportedModels(value.supported_models, { targetModel }),
    };
    for (const key of [
        'default_alias',
        'name',
        'version',
        'author',
        'author_url',
        'display_name',
        'description',
        'namespace',
        'owner',
        'repo',
        'relative_path',
        'write_block_reason',
    ]) {
        if (typeof entry[key] !== 'string')
            Reflect.deleteProperty(entry, key);
    }
    if (typeof entry.is_writable !== 'boolean')
        delete entry.is_writable;
    if (Array.isArray(value.tags))
        entry.tags = value.tags.filter((tag) => typeof tag === 'string');
    else
        delete entry.tags;
    for (const key of ['source', 'metadata']) {
        if (!isRecord(entry[key]))
            Reflect.deleteProperty(entry, key);
    }
    if (entry.lineage !== null && !isRecord(entry.lineage))
        delete entry.lineage;
    return entry;
}
