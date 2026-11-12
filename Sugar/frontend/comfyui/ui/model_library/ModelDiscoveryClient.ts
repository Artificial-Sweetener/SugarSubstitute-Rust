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
/** Address reviewed discovery and shared acquisition jobs through Comfy's host transport. */

import type { ModelTransferObserver } from './ModelTransfer.js';
import { ModelJobClient } from './ModelJobClient.js';
import { modelApiError } from './ModelApiTransport.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { decodeDiscoveryContext, decodeDiscoveryPlan } from './ModelDiscoveryCodec.js';
import type {
  ModelDiscoveryContext,
  ModelDiscoveryPlan,
  ModelDiscoveryPort,
} from './ModelDiscovery.js';

const base = '/sugarcubes/v1/models/discovery';

/** Share planning, cancellation, and exact publication evidence with every model consumer. */
export class ModelDiscoveryClient implements ModelDiscoveryPort {
  readonly #fetch: ModelApiFetch;
  readonly #jobs: ModelJobClient;
  readonly #installed: () => Promise<void>;

  /** Bind authoritative host refresh after a completed artifact publication. */
  constructor(fetchApi: ModelApiFetch, installed: () => Promise<void>) {
    this.#fetch = fetchApi;
    this.#jobs = new ModelJobClient(fetchApi);
    this.#installed = installed;
  }

  /** Ask the domain owner whether an explicit field context can offer discovery. */
  async context(
    kind: string,
    targetModel: string,
    signal: AbortSignal,
  ): Promise<ModelDiscoveryContext | null> {
    return decodeDiscoveryContext(
      await (await this.#post(base + '/context', { kind, targetModel }, signal)).json(),
    );
  }

  /** Observe one cancellable planning job, retaining every ordered provider offer. */
  async discover(
    kind: string,
    targetModel: string,
    publicOnly: boolean,
    progress: (message: string) => void,
    signal: AbortSignal,
  ): Promise<ModelDiscoveryPlan> {
    const response = await this.#post(base, { kind, targetModel, publicOnly }, signal);
    const result = await this.#jobs.observe(
      response,
      { queued: 'Finding compatible models…', running: 'Finding compatible models…' },
      progress,
      signal,
    );
    return decodeDiscoveryPlan(result);
  }

  /** Refresh Comfy only after verified publication and return the exact installed host filename. */
  async acquire(
    planId: string,
    identity: string,
    providerId: string,
    progress: (message: string) => void,
    signal: AbortSignal,
    transfer?: ModelTransferObserver,
  ): Promise<string> {
    const response = await this.#post(
      base + '/' + encodeURIComponent(planId) + '/acquire',
      { identity, providerId },
      signal,
    );
    const result = await this.#jobs.observe(
      response,
      { queued: 'Waiting to download…', running: 'Downloading model…' },
      progress,
      signal,
      transfer,
    );
    if (
      result.status !== 'installed' ||
      typeof result.fileName !== 'string' ||
      !result.fileName ||
      result.fileName.length > 1024
    )
      throw new Error('Model download returned invalid results.');
    await this.#installed();
    return result.fileName;
  }

  /** Release closed-presentation evidence independently of accepted shared transfer jobs. */
  async release(planId: string): Promise<void> {
    const response = await this.#fetch(base + '/' + encodeURIComponent(planId), {
      method: 'DELETE',
    });
    if (!response.ok) throw new Error('Model suggestions could not be released.');
  }

  /** Reject server command failures through contained presentation feedback. */
  async #post(
    path: string,
    value: Record<string, unknown>,
    signal: AbortSignal,
  ): Promise<Response> {
    const response = await this.#fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(value),
      signal,
    });
    if (!response.ok)
      throw await modelApiError(response, 'Model discovery is unavailable. Try again later.');
    return response;
  }
}
