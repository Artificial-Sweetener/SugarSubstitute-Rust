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
/** Keep credential HTTP commands separate from suggestion rendering and acquisition state. */

import { isRecord } from '../types/common.js';
import { modelApiError } from './ModelApiTransport.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import type { CivitaiCredentialPort, CivitaiCredentialStatus } from './ModelDiscovery.js';

const path = '/sugarcubes/v1/models/credentials/civitai';

/** Address secure host persistence without retaining keys in browser preferences or shared assets. */
export class CivitaiCredentialClient implements CivitaiCredentialPort {
  readonly #fetch: ModelApiFetch;

  /** Bind the same backend-addressing transport as the model catalog. */
  constructor(fetchApi: ModelApiFetch) {
    this.#fetch = fetchApi;
  }

  /** Read secret-free configured and storage capability state. */
  async status(signal: AbortSignal): Promise<CivitaiCredentialStatus> {
    return this.#read(await this.#fetch(path, { signal }));
  }

  /** Save only explicit user-entered material through the secure credential owner. */
  async save(key: string, signal: AbortSignal): Promise<CivitaiCredentialStatus> {
    return this.#read(
      await this.#fetch(path, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ apiKey: key }),
        signal,
      }),
    );
  }

  /** Validate status without exposing or accepting a returned credential value. */
  async #read(response: Response): Promise<CivitaiCredentialStatus> {
    if (!response.ok)
      throw await modelApiError(response, 'The CivitAI credential could not be saved.');
    const value: unknown = await response.json();
    if (
      !isRecord(value) ||
      value.schemaVersion !== 1 ||
      value.provider !== 'civitai' ||
      typeof value.configured !== 'boolean' ||
      typeof value.storageAvailable !== 'boolean' ||
      (value.storageReason !== null && typeof value.storageReason !== 'string')
    )
      throw new Error('Invalid CivitAI credential status.');
    return {
      configured: value.configured,
      storageAvailable: value.storageAvailable,
      storageReason: value.storageReason,
    };
  }
}
