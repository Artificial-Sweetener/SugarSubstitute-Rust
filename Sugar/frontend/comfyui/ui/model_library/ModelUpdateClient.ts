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
/** Share validated update state while leaving consent and eligibility on the backend. */
import type { ModelApiFetch } from './ModelApiTransport.js';
import { modelApiError } from './ModelApiTransport.js';
import { decodeModelUpdates } from './ModelUpdates.js';
import type { ModelUpdatePort, ModelUpdateSnapshot } from './ModelUpdates.js';

/** Keep update decisions separate from catalog values and native field callbacks. */
export class ModelUpdateClient implements ModelUpdatePort {
  readonly #fetch: ModelApiFetch;
  readonly #listeners = new Set<(snapshot: ModelUpdateSnapshot) => void>();
  #snapshot: ModelUpdateSnapshot = { enabled: false, proposals: [] };
  #checking: Promise<void> | null = null;
  readonly #selections = new Map<string, Promise<void>>();
  /** Bind the host HTTP adapter without owning focus or settings widgets. */
  constructor(fetch: ModelApiFetch) {
    this.#fetch = fetch;
  }
  /** Return the latest validated server snapshot. */
  read(): ModelUpdateSnapshot {
    return this.#snapshot;
  }
  /** Subscribe projected banners and tiles without per-field provider requests. */
  subscribe(listener: (snapshot: ModelUpdateSnapshot) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
  /** Load consent and existing proposals without starting provider traffic. */
  load(): Promise<void> {
    return this.#request('/sugarcubes/v1/models/updates');
  }
  /** Coalesce focus events, allowing the authoritative server to enforce opt-in and relevance. */
  check(): Promise<void> {
    this.#checking ??= this.#request('/sugarcubes/v1/models/updates/check', {
      method: 'POST',
    }).finally(() => {
      this.#checking = null;
    });
    return this.#checking;
  }
  /** Share selection requests while the backend owns persistent daily provider freshness. */
  checkSelection(modelId: string): Promise<void> {
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
  setEnabled(enabled: boolean): Promise<void> {
    return this.#request('/sugarcubes/v1/models/updates', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled }),
    });
  }
  /** Dismiss only the version currently offered for one catalog identity. */
  dismiss(modelId: string): Promise<void> {
    return this.#decision(modelId, 'dismiss');
  }
  /** Persist provider-page opt-out through a verified server-side proposal. */
  disableModel(modelId: string): Promise<void> {
    return this.#decision(modelId, 'disable');
  }
  /** Send an exact local identity instead of trusting provider IDs from presentation. */
  #decision(modelId: string, decision: 'dismiss' | 'disable'): Promise<void> {
    if (!/^[a-f0-9]{64}$/.test(modelId))
      return Promise.reject(new Error('Invalid model update identity.'));
    return this.#request(`/sugarcubes/v1/models/${modelId}/updates/${decision}`, {
      method: 'POST',
    });
  }
  /** Publish state only after HTTP and payload validation succeed. */
  async #request(path: string, options?: RequestInit): Promise<void> {
    const response = await this.#fetch(path, options);
    if (!response.ok) throw await modelApiError(response, 'Model updates are unavailable.');
    const snapshot = decodeModelUpdates(await response.json());
    this.#snapshot = snapshot;
    for (const listener of this.#listeners) listener(snapshot);
  }
}
