import { abortableModelDelay } from './ModelJobClient.js';
import { isRecord } from '../types/common.js';
/**
 * Share one change subscription between every catalog consumer.
 *
 * The server holds each change request open until the revision advances, so one request per
 * consumer would multiply idle connections. The poll starts with the first watcher and stops
 * only when the last watcher leaves; consumers joining or leaving never start another request.
 */
export class ModelCatalogSubscription {
    #fetch;
    #load;
    #watchers = new Set();
    #controller = null;
    /** Bind the catalog transport and the catalog owner's snapshot loader. */
    constructor(fetchApi, load) {
        this.#fetch = fetchApi;
        this.#load = load;
    }
    /** Receive committed generations until the returned release is called. */
    watch(listener, reportError) {
        const watcher = { listener, reportError };
        this.#watchers.add(watcher);
        if (!this.#controller) {
            const controller = new AbortController();
            this.#controller = controller;
            void this.#run(controller.signal);
        }
        return () => {
            if (!this.#watchers.delete(watcher) || this.#watchers.size)
                return;
            this.#controller?.abort();
            this.#controller = null;
        };
    }
    /** Follow revisions with bounded backoff, publishing each committed snapshot to every watcher. */
    async #run(signal) {
        let snapshot = null;
        let failures = 0;
        while (!signal.aborted) {
            try {
                snapshot ??= await this.#load(false);
                const response = await this.#fetch(`/sugarcubes/v1/models/changes?afterRevision=${String(snapshot.revision)}`, { signal });
                if (!response.ok)
                    throw new Error(`Model catalog updates are unavailable (${String(response.status)}).`);
                const payload = await response.json();
                if (!isRecord(payload) ||
                    payload.schemaVersion !== 1 ||
                    typeof payload.revision !== 'number' ||
                    !Number.isSafeInteger(payload.revision) ||
                    payload.revision < 0)
                    throw new Error('Invalid model catalog update response.');
                if (payload.revision !== snapshot.revision) {
                    snapshot = await this.#load(true);
                    if (!signal.aborted)
                        for (const watcher of [...this.#watchers])
                            watcher.listener(snapshot);
                }
                failures = 0;
            }
            catch (error) {
                if (signal.aborted)
                    return;
                if (failures === 0)
                    for (const watcher of [...this.#watchers])
                        watcher.reportError(error);
                failures += 1;
                try {
                    await abortableModelDelay(Math.min(30_000, 1_000 * 2 ** Math.min(failures - 1, 5)), signal);
                }
                catch {
                    return;
                }
            }
        }
    }
}
