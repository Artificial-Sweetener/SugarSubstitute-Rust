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
/** Queue canonical Cube workflows through SugarCubes' authoritative backend. */

import { isRecord } from '../../types/common.js';
import type { ApiResponse, ComfyHostApi } from '../../types/graph.js';

export interface CubeExecutionRequestPayload {
  schema_version: 1;
  workflow: unknown;
  proximity_connections: unknown[];
  queue: {
    client_id: string | null;
    front: boolean;
    number: number | null;
    partial_execution_targets: string[];
    atomic: boolean;
  };
  optimization: {
    enabled: boolean;
    bypass_empty_lazy_lora: boolean;
    intern_pure_values: boolean;
    intern_resource_streams: boolean;
  };
  extra_data: Record<string, unknown>;
}

/** Own transport and error normalization for direct Cube execution. */
export class CubeExecutionApiClient {
  constructor(private readonly api: ComfyHostApi) {}

  /** Queue one prepared workflow request and return Comfy-compatible response JSON. */
  async queue(request: CubeExecutionRequestPayload): Promise<unknown> {
    if (!this.api.fetchApi) throw new TypeError('Comfy fetchApi is unavailable.');
    const response: ApiResponse = await this.api.fetchApi('/sugarcubes/v2/executions/queue', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
    });
    const payload = await response.json();
    if (response.ok === false || !isRecord(payload) || payload.accepted !== true) {
      throw new CubeExecutionApiError(payload, response.status ?? 500);
    }
    return payload;
  }
}

/** Retain structured backend diagnostics for Comfy's queue caller. */
export class CubeExecutionApiError extends Error {
  constructor(
    readonly payload: unknown,
    readonly status: number,
  ) {
    super(readExecutionError(payload));
    this.name = 'CubeExecutionApiError';
  }
}

/** Read one useful diagnostic from either execution or repository error shape. */
function readExecutionError(payload: unknown): string {
  if (!isRecord(payload)) return 'SugarCubes execution failed.';
  const error = payload.error;
  if (typeof error === 'string') return error;
  if (isRecord(error)) {
    if (typeof error.message === 'string') return error.message;
    if (typeof error.type === 'string') return error.type;
  }
  return 'SugarCubes execution failed.';
}
