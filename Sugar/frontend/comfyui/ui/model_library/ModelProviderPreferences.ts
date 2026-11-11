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
/** Define and validate shared provider settings without persisting browser-owned policy. */
import { isRecord } from '../types/common.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { modelApiError } from './ModelApiTransport.js';

/** Keep stable reference policy identities beside their settings captions. */
export const modelThumbnailSafetyChoices = [
  { value: 'sfw_only', label: 'SFW only' },
  { value: 'allow_soft', label: 'Allow soft / racy' },
  { value: 'allow_all', label: 'Allow all' },
  { value: 'disabled', label: 'Disabled' },
] as const;
/** Admit only supported provider safety identities. */
export type ModelThumbnailSafety = (typeof modelThumbnailSafetyChoices)[number]['value'];

/** Describe the complete authoritative settings response independently of browser controls. */
export interface ModelProviderPreferences {
  readonly metadataLookupEnabled: boolean;
  readonly missingModelLookupEnabled: boolean;
  readonly downloadsEnabled: boolean;
  readonly thumbnailDownloadsEnabled: boolean;
  readonly thumbnailSafety: ModelThumbnailSafety;
}

/** Validate complete server snapshots before controls can display or edit shared policy. */
export function decodeModelProviderPreferences(value: unknown): ModelProviderPreferences {
  if (
    !isRecord(value) ||
    value.schemaVersion !== 1 ||
    typeof value.metadataLookupEnabled !== 'boolean' ||
    typeof value.missingModelLookupEnabled !== 'boolean' ||
    typeof value.downloadsEnabled !== 'boolean' ||
    typeof value.thumbnailDownloadsEnabled !== 'boolean'
  )
    throw new Error('The model provider settings response is invalid.');
  const safety = modelThumbnailSafetyChoices.find(
    (choice) => choice.value === value.thumbnailSafety,
  );
  if (!safety) throw new Error('The CivitAI thumbnail safety policy is invalid.');
  return {
    metadataLookupEnabled: value.metadataLookupEnabled,
    missingModelLookupEnabled: value.missingModelLookupEnabled,
    downloadsEnabled: value.downloadsEnabled,
    thumbnailDownloadsEnabled: value.thumbnailDownloadsEnabled,
    thumbnailSafety: safety.value,
  };
}

/** Use the shared HTTP owner for every preference read and deliberate partial update. */
export class ModelProviderPreferenceClient {
  readonly #fetch: ModelApiFetch;
  /** Bind the host-addressed fetch adapter independently of settings presentation. */
  constructor(fetchApi: ModelApiFetch) {
    this.#fetch = fetchApi;
  }
  /** Read current policy without performing remote metadata or model requests. */
  load(): Promise<ModelProviderPreferences> {
    return this.#request();
  }
  /** Preserve unrelated settings from other consumers by sending only explicit changes. */
  configure(patch: Partial<ModelProviderPreferences>): Promise<ModelProviderPreferences> {
    return this.#request({
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(patch),
    });
  }
  /** Bound settings transport and retain literal server feedback for the existing toast owner. */
  async #request(options?: RequestInit): Promise<ModelProviderPreferences> {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), 20000);
    try {
      const response = await this.#fetch('/sugarcubes/v1/models/preferences', {
        ...options,
        signal: controller.signal,
      });
      if (!response.ok)
        throw await modelApiError(response, 'Model provider settings are unavailable.');
      const payload: unknown = await response.json();
      return decodeModelProviderPreferences(payload);
    } finally {
      window.clearTimeout(timeout);
    }
  }
}
