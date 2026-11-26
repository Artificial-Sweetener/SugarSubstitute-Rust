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
/** Own the page's single long-poll of committed model catalog generations. */
import { decodeModelCatalogUpdate } from './ModelCatalog.js';
import { abortableModelDelay } from './ModelJobClient.js';
/**
 * Share one change subscription between every catalog consumer, and tell every one of them
 * about every new catalog, whoever read it.
 *
 * The server holds each change request open until the revision advances, so one request per
 * consumer would multiply idle connections. The poll starts with the first watcher and stops
 * only when the last watcher leaves; consumers joining or leaving never start another request.
 * A change names the models it touched, and only those are read.
 */
export class ModelCatalogSubscription {
    #fetch;
    #catalog;
    #watchers = new Set();
    #controller = null;
    #published = null;
    /** Bind the catalog transport and the catalog owner. */
    constructor(fetchApi, catalog) {
        this.#fetch = fetchApi;
        this.#catalog = catalog;
    }
    /** Receive every new catalog until the returned release is called. */
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
    /**
     * Tell every watcher about a catalog the owner now holds. The owner calls this for every
     * catalog it reads, so a change a person just made reaches every surface without waiting for
     * the long-poll to notice it. A catalog already published is not published again.
     */
    publish(snapshot) {
        if (snapshot === this.#published)
            return;
        this.#published = snapshot;
        for (const watcher of [...this.#watchers])
            watcher.listener(snapshot);
    }
    /** Follow revisions with bounded backoff; the owner publishes each catalog it takes. */
    async #run(signal) {
        let failures = 0;
        while (!signal.aborted) {
            try {
                const held = await this.#catalog.load(false);
                const response = await this.#fetch(`/sugarcubes/v1/models/changes?afterRevision=${String(held.revision)}`, { signal });
                if (!response.ok)
                    throw new Error(`Model catalog updates are unavailable (${String(response.status)}).`);
                const update = decodeModelCatalogUpdate(await response.json());
                if (update.changes)
                    this.#catalog.apply(update.changes);
                else if (update.revision !== held.revision)
                    await this.#catalog.load(true);
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
