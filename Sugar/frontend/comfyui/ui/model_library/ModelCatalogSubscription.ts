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
import type { ModelCatalogChanges, ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { abortableModelDelay } from './ModelJobClient.js';

interface CatalogWatcher {
  readonly listener: (snapshot: ModelCatalogSnapshot) => void;
  readonly reportError: (error: unknown) => void;
}

/** How the catalog owner takes what the subscription learned. */
export interface ModelCatalogFollower {
  /** Read the catalog, from the server when `force` is set. */
  load(force: boolean): Promise<ModelCatalogSnapshot>;
  /** Apply the models a change names to the held catalog. */
  apply(changes: ModelCatalogChanges): ModelCatalogSnapshot;
}

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
  readonly #fetch: ModelApiFetch;
  readonly #catalog: ModelCatalogFollower;
  readonly #watchers = new Set<CatalogWatcher>();
  #controller: AbortController | null = null;
  #published: ModelCatalogSnapshot | null = null;

  /** Bind the catalog transport and the catalog owner. */
  constructor(fetchApi: ModelApiFetch, catalog: ModelCatalogFollower) {
    this.#fetch = fetchApi;
    this.#catalog = catalog;
  }

  /** Receive every new catalog until the returned release is called. */
  watch(
    listener: (snapshot: ModelCatalogSnapshot) => void,
    reportError: (error: unknown) => void,
  ): () => void {
    const watcher: CatalogWatcher = { listener, reportError };
    this.#watchers.add(watcher);
    if (!this.#controller) {
      const controller = new AbortController();
      this.#controller = controller;
      void this.#run(controller.signal);
    }
    return () => {
      if (!this.#watchers.delete(watcher) || this.#watchers.size) return;
      this.#controller?.abort();
      this.#controller = null;
    };
  }

  /**
   * Tell every watcher about a catalog the owner now holds. The owner calls this for every
   * catalog it reads, so a change a person just made reaches every surface without waiting for
   * the long-poll to notice it. A catalog already published is not published again.
   */
  publish(snapshot: ModelCatalogSnapshot): void {
    if (snapshot === this.#published) return;
    this.#published = snapshot;
    for (const watcher of [...this.#watchers]) watcher.listener(snapshot);
  }

  /** Follow revisions with bounded backoff; the owner publishes each catalog it takes. */
  async #run(signal: AbortSignal): Promise<void> {
    let failures = 0;
    while (!signal.aborted) {
      try {
        const held = await this.#catalog.load(false);
        const response = await this.#fetch(
          `/sugarcubes/v1/models/changes?afterRevision=${String(held.revision)}`,
          { signal },
        );
        if (!response.ok)
          throw new Error(`Model catalog updates are unavailable (${String(response.status)}).`);
        const update = decodeModelCatalogUpdate(await response.json());
        if (update.changes) this.#catalog.apply(update.changes);
        else if (update.revision !== held.revision) await this.#catalog.load(true);
        failures = 0;
      } catch (error: unknown) {
        if (signal.aborted) return;
        if (failures === 0) for (const watcher of [...this.#watchers]) watcher.reportError(error);
        failures += 1;
        try {
          await abortableModelDelay(
            Math.min(30_000, 1_000 * 2 ** Math.min(failures - 1, 5)),
            signal,
          );
        } catch {
          return;
        }
      }
    }
  }
}
