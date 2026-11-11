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
/** Adapt bounded Comfy API requests to the canonical Cube graph gateway. */
import type { ApiJsonResult } from '../../core/CubeLibraryApi.js';
import { isRecord } from '../../types/common.js';
import type {
  CubeGraphAnalysis,
  CubeGraphGateway,
  CubeGraphSnapshot,
} from '../domain/CubeGraphModels.js';
import { readCubeGraphAnalysis } from './CubeGraphResponse.js';
import type { CubeGraphStackEntry, CubeGraphStackGateway } from '../domain/CubeGraphStackModels.js';

interface CubeGraphApiTransport {
  fetchJson(url: string, options?: RequestInit): Promise<ApiJsonResult>;
}

/** Retain topology authority on the backend while exposing typed browser operations. */
export class CubeGraphApi implements CubeGraphGateway, CubeGraphStackGateway {
  readonly #transport: CubeGraphApiTransport;

  /** Bind the shared Comfy transport rather than constructing another network client. */
  constructor(transport: CubeGraphApiTransport) {
    this.#transport = transport;
  }

  /** Analyze one exact snapshot without changing the live workflow. */
  analyze(snapshot: CubeGraphSnapshot): Promise<CubeGraphAnalysis> {
    return this.#request('analyze', snapshot);
  }

  /** Submit an exact segment permutation for canonical validation and geometry planning. */
  reorder(
    snapshot: CubeGraphSnapshot,
    segmentInstanceIds: readonly string[],
    orderedInstanceIds: readonly string[],
  ): Promise<CubeGraphAnalysis> {
    return this.#request('reorder', snapshot, {
      segment_instance_ids: segmentInstanceIds,
      ordered_instance_ids: orderedInstanceIds,
    });
  }

  /** Plan the complete cart without changing live host nodes before validation succeeds. */
  applyStack(
    snapshot: CubeGraphSnapshot,
    entries: readonly CubeGraphStackEntry[],
  ): Promise<CubeGraphAnalysis> {
    return this.#request('stack', snapshot, {
      entries: entries.map((entry) =>
        entry.source === 'existing'
          ? { source: entry.source, instance_id: entry.instanceId }
          : {
              source: entry.source,
              instance_id: entry.instanceId,
              alias: entry.alias,
              document: entry.document,
              size: entry.size,
            },
      ),
    });
  }

  /** Bound transport lifetime and reject failures before exposing any graph facts. */
  async #request(
    operation: 'analyze' | 'reorder' | 'stack',
    snapshot: CubeGraphSnapshot,
    fields: Record<string, unknown> = {},
  ): Promise<CubeGraphAnalysis> {
    const workflow: unknown = JSON.parse(snapshot.serializedWorkflow);
    if (!isRecord(workflow) || Array.isArray(workflow))
      throw new TypeError('Cube graph snapshot must be an object.');
    const abort = new AbortController();
    const timeout = globalThis.setTimeout(() => abort.abort(), 15000);
    try {
      const result = await this.#transport.fetchJson(`/sugarcubes/v2/workflows/${operation}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        signal: abort.signal,
        body: JSON.stringify({ schema_version: 1, workflow, ...fields }),
      });
      if (abort.signal.aborted) throw new Error(`Cube graph ${operation} timed out.`);
      if (result.response.status !== 200) {
        const error = isRecord(result.data.error) ? result.data.error : {};
        const message = typeof error.message === 'string' ? error.message.trim() : '';
        throw new Error(
          message ||
            `Cube graph ${operation} failed (${String(result.response.status ?? 'unknown')}).`,
        );
      }
      return readCubeGraphAnalysis(result.data);
    } finally {
      globalThis.clearTimeout(timeout);
    }
  }
}
