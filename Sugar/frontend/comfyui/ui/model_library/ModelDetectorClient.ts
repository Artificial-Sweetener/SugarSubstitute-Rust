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
/** Coalesce server-owned detector resolution without copying matching policy into the browser. */
import { decodeModelCatalog } from './ModelCatalog.js';
import type { ModelCatalogItem } from './ModelCatalog.js';
import type { ModelDetectorPort } from './ModelDetectors.js';
import { modelApiError } from './ModelApiTransport.js';
import type { ModelApiFetch } from './ModelApiTransport.js';

/** Share validated detector choices across every mounted field and invalidate them by catalog revision. */
export class ModelDetectorClient implements ModelDetectorPort {
  readonly #fetch: ModelApiFetch;
  readonly #choices = new Map<string, Promise<readonly ModelCatalogItem[]>>();
  #revision = -1;
  /** Bind only Comfy's configured API transport and keep caches scoped to catalog generations. */
  constructor(fetchApi: ModelApiFetch) {
    this.#fetch = fetchApi;
  }

  /** Resolve exact host choices, including recipes that have no installed catalog record. */
  resolve(values: readonly string[], revision: number): Promise<readonly ModelCatalogItem[]> {
    if (revision !== this.#revision) {
      this.#choices.clear();
      this.#revision = revision;
    }
    const key = JSON.stringify(values);
    const prior = this.#choices.get(key);
    if (prior) return prior;
    const pending = this.#read('/sugarcubes/v1/models/detectors/resolve', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ values }),
    })
      .then((items) => {
        if (
          items.length !== values.length ||
          items.some((item, index) => item.kind !== 'ultralytics' || item.value !== values[index])
        )
          throw new Error('Invalid detector model choices.');
        return items;
      })
      .catch((error: unknown) => {
        if (this.#choices.get(key) === pending) this.#choices.delete(key);
        throw error;
      });
    this.#choices.set(key, pending);
    return pending;
  }

  /** Read the same complete curated visual library used by the desktop consumer. */
  library(signal: AbortSignal): Promise<readonly ModelCatalogItem[]> {
    return this.#read('/sugarcubes/v1/models/detectors/library', { signal });
  }

  /** Save only the exact backend value and an approved server library asset name. */
  async assign(value: string, asset: string, signal: AbortSignal): Promise<void> {
    const response = await this.#fetch('/sugarcubes/v1/models/detectors/association', {
      method: 'PUT',
      signal,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value, asset }),
    });
    if (!response.ok)
      throw await modelApiError(response, 'The detector thumbnail could not be saved.');
    this.#choices.clear();
  }

  /** Validate prepared image references and labels before publishing any dynamic UI data. */
  async #read(path: string, options: RequestInit): Promise<readonly ModelCatalogItem[]> {
    const response = await this.#fetch(path, options);
    if (!response.ok) throw await modelApiError(response, 'Detector thumbnails are unavailable.');
    return decodeModelCatalog(await response.json()).items;
  }
}
