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
/** Call the additive SugarScript authoring endpoint through Comfy's API. */

import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import { readSugarScriptAuthoringResponse } from './SugarScriptAuthoringModels.js';
import type { SugarScriptAuthoringResponse } from './SugarScriptAuthoringModels.js';
import { isRecord } from '../types/common.js';
import {
  readExactIntegerFields,
  type ExactIntegerField,
} from '../workflow/ExactIntegerRelations.js';

/** One compiled script: its plan, its workflow, and the whole numbers a page would round. */
export interface CompiledSugarScript extends SugarScriptAuthoringResponse {
  readonly workflow: Record<string, unknown> | null;
  readonly exactIntegers: readonly ExactIntegerField[];
}

/** Own transport and response validation for source-to-native authoring. */
export class SugarScriptAuthoringApi {
  readonly #api: CubeLibraryApi;

  /** Reuse SugarCubes' standard Comfy API adapter. */
  constructor(api: CubeLibraryApi) {
    this.#api = api;
  }

  /**
   * Compile bounded source without mutating or queueing a workflow.
   *
   * The reply also carries the compiled workflow, whose metadata records the script's editor
   * state, and names each whole number this page cannot hold, with its digits.
   */
  async compile(source: string): Promise<CompiledSugarScript> {
    const result = await this.#api.fetchJson('/sugarcubes/v2/sugarscript/compile', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ source }),
    });
    if (result.response.status !== 200 && result.response.status !== 422) {
      throw new Error(readBackendError(result.data, result.response.status));
    }
    const workflow = 'workflow' in result.data ? result.data.workflow : null;
    return {
      ...readSugarScriptAuthoringResponse(result.data),
      workflow: isRecord(workflow) ? workflow : null,
      exactIntegers: readExactIntegerFields(
        'exact_integers' in result.data ? result.data.exact_integers : undefined,
      ),
    };
  }

  /**
   * Write the project script of one serialized workflow, without storing it, and name the
   * mask files its loaders read.
   */
  async render(
    workflow: unknown,
  ): Promise<{ source: string; maskFiles: readonly { file: string; name: string }[] }> {
    const result = await this.#api.fetchJson('/sugarcubes/v2/sugarscript/render', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ workflow }),
    });
    const source = 'source' in result.data ? result.data.source : null;
    if (result.response.status !== 200 || typeof source !== 'string')
      throw new Error(readBackendError(result.data, result.response.status, 'writing'));
    const listed: unknown = 'mask_files' in result.data ? result.data.mask_files : [];
    return {
      source,
      maskFiles: (Array.isArray(listed) ? (listed as unknown[]) : []).flatMap((mask) =>
        isRecord(mask) && typeof mask.file === 'string' && typeof mask.name === 'string'
          ? [{ file: mask.file, name: mask.name }]
          : [],
      ),
    };
  }
}

/** Keep expected backend failures useful without trusting arbitrary response data. */
function readBackendError(
  value: object,
  status: number | undefined,
  action: 'compilation' | 'writing' = 'compilation',
): string {
  const error =
    'error' in value && typeof value.error === 'object' && value.error ? value.error : {};
  const message =
    'message' in error && typeof error.message === 'string' ? error.message.trim() : '';
  return message || `SugarScript ${action} failed${status ? ` (${String(status)})` : ''}.`;
}
