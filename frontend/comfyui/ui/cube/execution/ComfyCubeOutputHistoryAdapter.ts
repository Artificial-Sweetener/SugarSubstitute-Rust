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
/** Reconcile prompt-only Cube output results from Comfy's execution history. */

import { isRecord } from '../../types/common.js';
import type { CubeOutputExecutionStore } from './CubeOutputExecutionStore.js';

interface HistoryResponse {
  ok: boolean;
  json(): Promise<unknown>;
}

export type ComfyExecutionSuccessListener = (event: unknown) => void | Promise<void>;

export interface ComfyCubeOutputHistoryApi {
  addEventListener(name: 'execution_success', listener: ComfyExecutionSuccessListener): void;
  removeEventListener(name: 'execution_success', listener: ComfyExecutionSuccessListener): void;
  fetchApi(path: string): Promise<HistoryResponse>;
}

export interface ComfyCubeOutputHistoryAdapterOptions {
  api: ComfyCubeOutputHistoryApi;
  store: CubeOutputExecutionStore;
  logger?: Pick<Console, 'debug' | 'error'>;
}

/**
 * Own history reconciliation for Cube outputs omitted from Comfy's node store.
 *
 * This path covers cached output nodes and page/workflow restoration, where no
 * usable `executed` message is available but Comfy history remains authoritative.
 */
export class ComfyCubeOutputHistoryAdapter {
  readonly #api: ComfyCubeOutputHistoryApi;
  readonly #store: CubeOutputExecutionStore;
  readonly #logger: Pick<Console, 'debug' | 'error'> | null;
  #installed = false;

  /** Bind Comfy history and completion events to the Cube result store. */
  constructor(options: ComfyCubeOutputHistoryAdapterOptions) {
    this.#api = options.api;
    this.#store = options.store;
    this.#logger = options.logger ?? null;
  }

  /** Install one stable completion listener. */
  install(): void {
    if (this.#installed) return;
    this.#api.addEventListener('execution_success', this.#handleExecutionSuccess);
    this.#installed = true;
  }

  /** Remove only this adapter's completion listener. */
  dispose(): void {
    if (!this.#installed) return;
    this.#api.removeEventListener('execution_success', this.#handleExecutionSuccess);
    this.#installed = false;
  }

  /** Reconcile one completed prompt from Comfy's authoritative history entry. */
  async reconcilePrompt(promptId: string): Promise<number> {
    const cleanedPromptId = promptId.trim();
    if (!cleanedPromptId) return 0;
    return await this.#fetchAndRetain(`/history/${encodeURIComponent(cleanedPromptId)}`);
  }

  /** Restore recent Cube results after page or workflow reconfiguration. */
  async hydrateRecent(): Promise<number> {
    return await this.#fetchAndRetain('/history?max_items=50');
  }

  /** Load and validate history without allowing host failures to escape events. */
  async #fetchAndRetain(path: string): Promise<number> {
    try {
      const response = await this.#api.fetchApi(path);
      if (!response.ok) {
        throw new Error(`Comfy history request failed for ${path}.`);
      }
      const payload = await response.json();
      const retained = retainHistoryOutputs(payload, this.#store);
      if (retained > 0) {
        this.#logger?.debug('SugarCubes reconciled loaded Cube output history.', {
          path,
          retained,
        });
      }
      return retained;
    } catch (error: unknown) {
      this.#logger?.error('SugarCubes failed to reconcile Cube output history.', {
        path,
        reason: error instanceof Error ? error.message : String(error),
        error,
      });
      return 0;
    }
  }

  /** Reconcile the exact prompt only after Comfy reports successful completion. */
  readonly #handleExecutionSuccess: ComfyExecutionSuccessListener = async (event) => {
    if (!isRecord(event) || !isRecord(event.detail)) return;
    const promptId = event.detail.prompt_id;
    if (typeof promptId !== 'string' || !promptId.trim()) return;
    await this.reconcilePrompt(promptId);
  };
}

/** Retain Cube-only entries while preserving Comfy's oldest-to-newest history order. */
function retainHistoryOutputs(payload: unknown, store: CubeOutputExecutionStore): number {
  if (!isRecord(payload)) return 0;
  let retained = 0;
  for (const historyEntry of Object.values(payload)) {
    if (!isRecord(historyEntry) || !isRecord(historyEntry.outputs)) continue;
    for (const [executionId, output] of Object.entries(historyEntry.outputs)) {
      if (store.retain(executionId, output)) retained += 1;
    }
  }
  return retained;
}
