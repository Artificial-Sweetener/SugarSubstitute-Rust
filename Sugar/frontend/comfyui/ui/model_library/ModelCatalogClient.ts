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
/** Coalesce prepared catalog requests independently of node cards and picker popups. */
import type { ModelTransferObserver } from './ModelTransfer.js';
import { CivitaiCredentialClient } from './CivitaiCredentialClient.js';
import {
  applyModelCatalogChanges,
  continueModelCatalog,
  decodeModelCatalog,
} from './ModelCatalog.js';
import type { ModelCatalogChanges, ModelCatalogSnapshot } from './ModelCatalog.js';
import type { ModelCatalogItem } from './ModelCatalog.js';
import { isRecord } from '../types/common.js';
import type { ModelPickerActions } from './ModelPickerActions.js';

import { modelApiError } from './ModelApiTransport.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { ModelJobClient } from './ModelJobClient.js';
import { ModelCatalogSubscription } from './ModelCatalogSubscription.js';
import { decodeModelReleases } from './ModelReleases.js';
import type { ModelRelease } from './ModelReleases.js';

/** Share one validated snapshot across every graph and masonry model field. */
export class ModelCatalogClient implements ModelPickerActions {
  readonly credentials: CivitaiCredentialClient;
  readonly #fetch: ModelApiFetch;
  readonly #jobs: ModelJobClient;
  readonly #installed: (() => Promise<void>) | null;
  readonly #changes: ModelCatalogSubscription;
  #pending: Promise<ModelCatalogSnapshot> | null = null;
  #snapshot: ModelCatalogSnapshot | null = null;
  #prioritized = '';
  readonly #namedOnce = new Set<string>();

  /** Bind Comfy's backend-addressing adapter rather than assuming a localhost origin. */
  constructor(fetchApi: ModelApiFetch, installed: (() => Promise<void>) | null = null) {
    this.#fetch = fetchApi;
    this.credentials = new CivitaiCredentialClient(fetchApi);
    this.#jobs = new ModelJobClient(fetchApi);
    this.#installed = installed;
    this.#changes = new ModelCatalogSubscription(fetchApi, {
      load: (force) => this.load(force),
      apply: (changes) => this.#apply(changes),
    });
  }

  /** Expose the prepared catalog without starting IO from consumer interaction paths. */
  current(): ModelCatalogSnapshot | null {
    return this.#snapshot;
  }

  /** Persist a user classification independently of provider facts and refresh prepared metadata. */
  async classifyContent(modelId: string, nsfw: boolean | null, signal: AbortSignal): Promise<void> {
    const response = await this.#fetch(
      `/sugarcubes/v1/models/${encodeURIComponent(modelId)}/content`,
      {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ nsfw }),
        signal,
      },
    );
    if (!response.ok)
      throw await modelApiError(response, 'The content classification could not be saved.');
    await this.load(true);
  }

  /** Read the shared downloader pattern through the backend preference owner. */
  async downloadPattern(): Promise<string> {
    const response = await this.#fetch('/sugarcubes/v1/models/download-preferences');
    if (!response.ok) throw await modelApiError(response, 'Download organization is unavailable.');
    const payload: unknown = await response.json();
    if (!isRecord(payload) || payload.schemaVersion !== 1 || typeof payload.pattern !== 'string')
      throw new Error('Invalid download organization settings.');
    return payload.pattern;
  }

  /** Save a relative organizing pattern without changing existing model paths. */
  async saveDownloadPattern(pattern: string): Promise<void> {
    const response = await this.#fetch('/sugarcubes/v1/models/download-preferences', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ pattern }),
    });
    if (!response.ok)
      throw await modelApiError(response, 'The download organization could not be saved.');
  }

  /** Reuse a completed snapshot or await the same in-flight HTTP request. */
  load(refresh = false): Promise<ModelCatalogSnapshot> {
    if (this.#pending) return this.#pending;
    if (this.#snapshot && !refresh) return Promise.resolve(this.#snapshot);
    this.#pending = this.#read().finally(() => {
      this.#pending = null;
    });
    return this.#pending;
  }

  /** Request one explicit background stat scan through the catalog owner. */
  async refresh(): Promise<void> {
    const response = await this.#fetch('/sugarcubes/v1/models/refresh', { method: 'POST' });
    if (!response.ok) throw new Error(`Model library refresh failed (${String(response.status)}).`);
    this.#snapshot = null;
  }

  /**
   * Give the models a person is looking at priority, without changing values or creating
   * client-owned jobs. Only a model still waiting for its lookup or its image is named: naming
   * one that is owed nothing changes nothing, so a finished library sends no requests.
   *
   * A wall names what is on screen `again` whenever that changes, because the server keeps the
   * most recently named models first; only an immediate repeat is skipped. A surface that names
   * the same models every time the catalog changes names each waiting model `once`, so several
   * such surfaces with different models never trade places request after request.
   */
  async prioritize(
    items: readonly ModelCatalogItem[],
    repeat: 'again' | 'once' = 'again',
  ): Promise<void> {
    const waiting = new Set(
      items
        .filter((item) => item.image?.state === 'pending' || item.image?.state === 'retrying')
        .map((item) => item.id),
    );
    // A model that stopped waiting may be named once more if it ever waits again.
    for (const item of items) if (!waiting.has(item.id)) this.#namedOnce.delete(item.id);
    const identities = [...waiting]
      .filter((id) => /^[a-f0-9]{64}$/.test(id))
      .filter((id) => repeat === 'again' || !this.#namedOnce.has(id))
      .slice(0, 128);
    const named = identities.join(' ');
    if (!identities.length || (repeat === 'again' && named === this.#prioritized)) return;
    if (repeat === 'again') this.#prioritized = named;
    else for (const id of identities) this.#namedOnce.add(id);
    try {
      const response = await this.#fetch('/sugarcubes/v1/models/metadata/prioritize', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ modelIds: identities }),
      });
      if (!response.ok) throw new Error('Background model presentation could not be prioritized.');
    } catch (error: unknown) {
      if (repeat === 'again' && this.#prioritized === named) this.#prioritized = '';
      else if (repeat === 'once') for (const id of identities) this.#namedOnce.delete(id);
      throw error;
    }
  }

  /** Observe the server-owned job and cancel it when its requesting presentation closes. */
  async refreshMetadata(
    modelId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
  ): Promise<void> {
    const response = await this.#fetch(
      `/sugarcubes/v1/models/${encodeURIComponent(modelId)}/metadata/refresh`,
      { method: 'POST', signal },
    );
    if (!response.ok) throw new Error('Model metadata refresh could not be started.');
    const result = await this.#jobs.observe(
      response,
      {
        queued: 'Waiting to refresh metadata…',
        running: 'Reading fingerprint and refreshing metadata…',
      },
      progress,
      signal,
    );
    progress(
      typeof result.warning === 'string'
        ? result.warning
        : result.status === 'not_found'
          ? result.kept === true
            ? 'Not found. Your card is unchanged.'
            : 'Not found.'
          : 'Metadata refreshed.',
    );
    await this.load(true);
  }

  /** Upload only selected image bytes; model paths and remote addresses remain server-owned. */
  async replaceThumbnail(modelId: string, image: File, signal: AbortSignal): Promise<void> {
    if (!image.size || image.size > 20 * 1024 * 1024)
      throw new Error('Choose an image smaller than 20 MiB.');
    const response = await this.#fetch(
      `/sugarcubes/v1/models/${encodeURIComponent(modelId)}/thumbnail`,
      { method: 'PUT', body: image, signal },
    );
    if (!response.ok) throw new Error('The model preview could not be saved.');
    await this.load(true);
  }

  /** Read provider history without changing the field or initiating a model fingerprint. */
  async releases(modelId: string, signal: AbortSignal): Promise<readonly ModelRelease[]> {
    const response = await this.#fetch(
      `/sugarcubes/v1/models/${encodeURIComponent(modelId)}/versions`,
      { signal },
    );
    if (!response.ok)
      throw await modelApiError(response, 'Model versions are unavailable. Try again later.');
    return decodeModelReleases(await response.json());
  }

  /** Acquire only server-offered artifact identities and refresh host choices after publication. */
  async acquire(
    modelId: string,
    versionId: string,
    fileId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
    transfer?: ModelTransferObserver,
  ): Promise<string> {
    const response = await this.#fetch(
      `/sugarcubes/v1/models/${encodeURIComponent(modelId)}/acquire`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ versionId, fileId }),
        signal,
      },
    );
    if (!response.ok)
      throw await modelApiError(response, 'The model download could not be started.');
    const result = await this.#jobs.observe(
      response,
      { queued: 'Waiting to download…', running: 'Downloading and verifying model…' },
      progress,
      signal,
      transfer,
    );
    if (result.status !== 'installed' || typeof result.fileName !== 'string')
      throw new Error('Invalid model download result.');
    await this.#installed?.();
    await this.load(true);
    return result.fileName;
  }

  /** Follow every catalog this owner comes to hold, from the page's one change subscription. */
  watch(
    listener: (snapshot: ModelCatalogSnapshot) => void,
    reportError: (error: unknown) => void,
  ): () => void {
    return this.#changes.watch(listener, reportError);
  }

  /** Validate unknown JSON before publishing it to any picker consumer. */
  async #read(): Promise<ModelCatalogSnapshot> {
    const held = this.#snapshot;
    // Naming the revision already held lets the server answer without resending the catalog.
    const response = await this.#fetch(
      '/sugarcubes/v1/models',
      held ? { headers: { 'X-SugarCubes-Held-Revision': String(held.revision) } } : undefined,
    );
    if (!response.ok) throw new Error(`Model catalog is unavailable (${String(response.status)}).`);
    const payload: unknown = await response.json();
    if (held && isUnchangedCatalog(payload, held.revision)) {
      const scanning = payload.scan.status === 'queued' || payload.scan.status === 'running';
      return this.#hold(scanning === held.scanning ? held : { ...held, scanning });
    }
    return this.#hold(continueModelCatalog(held, decodeModelCatalog(payload)));
  }

  /** Take the models a published change names without reading the rest of the catalog. */
  #apply(changes: ModelCatalogChanges): ModelCatalogSnapshot {
    const held = this.#snapshot;
    if (!held) throw new Error('A model catalog change arrived before its catalog.');
    return this.#hold(applyModelCatalogChanges(held, changes));
  }

  /**
   * Hold a catalog and tell every watching surface about it. Every catalog passes through here,
   * whoever asked for it, so no surface keeps showing the one before.
   */
  #hold(snapshot: ModelCatalogSnapshot): ModelCatalogSnapshot {
    this.#snapshot = snapshot;
    this.#changes.publish(snapshot);
    return snapshot;
  }
}

/** The server's answer when the revision this client holds is still current. */
function isUnchangedCatalog(
  value: unknown,
  revision: number,
): value is { readonly scan: { readonly status: unknown } } {
  if (typeof value !== 'object' || value === null) return false;
  const record = value as Record<string, unknown>;
  return (
    record.unchanged === true &&
    record.revision === revision &&
    typeof record.scan === 'object' &&
    record.scan !== null
  );
}
