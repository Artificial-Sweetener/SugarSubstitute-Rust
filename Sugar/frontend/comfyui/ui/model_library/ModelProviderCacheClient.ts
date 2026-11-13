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
/** Validate provider cache evidence and share the existing job observation lifecycle. */
import { isRecord } from '../types/common.js';
import { modelApiError, type ModelApiFetch } from './ModelApiTransport.js';
import { ModelJobClient } from './ModelJobClient.js';

export type ModelProviderCacheAction = 'clear_thumbnails' | 'clear_metadata' | 'refresh';

/** Describe provider-specific records and physical prepared image usage. */
export interface ModelProviderCacheSummary {
  readonly providerRecordCount: number;
  readonly thumbnailSourceCount: number;
  readonly thumbnailVariantCount: number;
  readonly thumbnailBytes: number;
}

/** Reject malformed usage rather than inventing an empty cache display. */
export function decodeModelProviderCacheSummary(value: unknown): ModelProviderCacheSummary {
  if (!isRecord(value) || value.schemaVersion !== 1)
    throw new Error('Invalid model cache usage response.');
  const count = (key: string): number => {
    const entry = value[key];
    if (typeof entry !== 'number' || !Number.isSafeInteger(entry) || entry < 0)
      throw new Error('Invalid model cache usage count.');
    return entry;
  };
  return {
    providerRecordCount: count('providerRecordCount'),
    thumbnailSourceCount: count('thumbnailSourceCount'),
    thumbnailVariantCount: count('thumbnailVariantCount'),
    thumbnailBytes: count('thumbnailBytes'),
  };
}

/** Address one server cache owner without browser-local metadata or image stores. */
export class ModelProviderCacheClient {
  readonly #fetch: ModelApiFetch;
  readonly #jobs: ModelJobClient;

  /** Reuse the same host fetch and job adapter as model acquisition and enrichment. */
  constructor(fetchApi: ModelApiFetch) {
    this.#fetch = fetchApi;
    this.#jobs = new ModelJobClient(fetchApi);
  }

  /** Read bounded usage without triggering model scans or provider requests. */
  async load(): Promise<ModelProviderCacheSummary> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20000);
    try {
      const response = await this.#fetch('/sugarcubes/v1/models/cache/civitai', {
        signal: controller.signal,
      });
      if (!response.ok) throw await modelApiError(response, 'Model cache usage is unavailable.');
      const payload: unknown = await response.json();
      return decodeModelProviderCacheSummary(payload);
    } finally {
      clearTimeout(timer);
    }
  }

  /** Observe an explicit action to completion before showing its committed usage evidence. */
  async execute(
    action: ModelProviderCacheAction,
    progress: (message: string) => void,
  ): Promise<ModelProviderCacheSummary> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 120000);
    try {
      const response = await this.#fetch(`/sugarcubes/v1/models/cache/civitai/${action}`, {
        method: 'POST',
        signal: controller.signal,
      });
      if (!response.ok)
        throw await modelApiError(response, 'Model cache action could not be started.');
      const payload = await this.#jobs.observe(
        response,
        {
          queued: 'Waiting for model cache work…',
          running: 'Updating model cache…',
        },
        progress,
        controller.signal,
      );
      return decodeModelProviderCacheSummary(payload);
    } finally {
      clearTimeout(timer);
    }
  }
}
