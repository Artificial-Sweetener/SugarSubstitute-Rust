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
/** Call exact-version persisted-workflow reconciliation through Comfy's API. */

import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import { readSugarScriptAuthoringResponse } from '../sugarscript/SugarScriptAuthoringModels.js';
import type { SugarScriptAuthoringResponse } from '../sugarscript/SugarScriptAuthoringModels.js';

/** Own transport and validation for legacy workflow-to-native authoring. */
export class LegacyWorkflowAuthoringApi {
  readonly #api: CubeLibraryApi;

  /** Reuse SugarCubes' standard Comfy API adapter. */
  constructor(api: CubeLibraryApi) {
    this.#api = api;
  }

  /** Reconcile one persisted workflow without mutating or queueing a graph. */
  async compile(workflow: Record<string, unknown>): Promise<SugarScriptAuthoringResponse> {
    const result = await this.#api.fetchJson('/sugarcubes/v2/workflows/compile-legacy', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workflow }),
    });
    if (result.response.status !== 200) {
      throw new Error(readBackendError(result.data, result.response.status));
    }
    return readSugarScriptAuthoringResponse(result.data);
  }
}

/** Keep exact-version reconciliation failures actionable at the UI boundary. */
function readBackendError(value: object, status: number | undefined): string {
  const error =
    'error' in value && typeof value.error === 'object' && value.error ? value.error : {};
  const message =
    'message' in error && typeof error.message === 'string' ? error.message.trim() : '';
  return message || `Cube workflow reconciliation failed${status ? ` (${String(status)})` : ''}.`;
}
