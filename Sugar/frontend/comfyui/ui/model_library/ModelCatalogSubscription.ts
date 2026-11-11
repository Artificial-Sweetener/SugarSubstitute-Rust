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
import type { ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { abortableModelDelay } from './ModelJobClient.js';
import { isRecord } from '../types/common.js';

interface CatalogWatcher {
  readonly listener: (snapshot: ModelCatalogSnapshot) => void;
  readonly reportError: (error: unknown) => void;
}

/**
 * Share one change subscription between every catalog consumer.
 *
 * The server holds each change request open until the revision advances, so one request per
 * consumer would multiply idle connections. The poll starts with the first watcher and stops
 * only when the last watcher leaves; consumers joining or leaving never start another request.
 */
export class ModelCatalogSubscription {
  readonly #fetch: ModelApiFetch;
  readonly #load: (force: boolean) => Promise<ModelCatalogSnapshot>;
  readonly #watchers = new Set<CatalogWatcher>();
  #controller: AbortController | null = null;

  /** Bind the catalog transport and the catalog owner's snapshot loader. */
  constructor(fetchApi: ModelApiFetch, load: (force: boolean) => Promise<ModelCatalogSnapshot>) {
    this.#fetch = fetchApi;
    this.#load = load;
  }

  /** Receive committed generations until the returned release is called. */
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

  /** Follow revisions with bounded backoff, publishing each committed snapshot to every watcher. */
  async #run(signal: AbortSignal): Promise<void> {
    let snapshot: ModelCatalogSnapshot | null = null;
    let failures = 0;
    while (!signal.aborted) {
      try {
        snapshot ??= await this.#load(false);
        const response = await this.#fetch(
          `/sugarcubes/v1/models/changes?afterRevision=${String(snapshot.revision)}`,
          { signal },
        );
        if (!response.ok)
          throw new Error(`Model catalog updates are unavailable (${String(response.status)}).`);
        const payload: unknown = await response.json();
        if (
          !isRecord(payload) ||
          payload.schemaVersion !== 1 ||
          typeof payload.revision !== 'number' ||
          !Number.isSafeInteger(payload.revision) ||
          payload.revision < 0
        )
          throw new Error('Invalid model catalog update response.');
        if (payload.revision !== snapshot.revision) {
          snapshot = await this.#load(true);
          if (!signal.aborted)
            for (const watcher of [...this.#watchers]) watcher.listener(snapshot);
        }
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
