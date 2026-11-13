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
/** Coordinate persisted-workflow reconciliation and native materialization. */

import type { Vec2 } from '../types/common.js';
import type { LegacyWorkflowAuthoringApi } from './LegacyWorkflowAuthoringApi.js';
import type {
  NativeWorkflowMaterializationResult,
  NativeWorkflowMaterializer,
} from './NativeWorkflowMaterializer.js';

/** Keep legacy workflow loading behind exact-version reconciliation. */
export class LegacyWorkflowImportService {
  readonly #api: LegacyWorkflowAuthoringApi;
  readonly #materializer: NativeWorkflowMaterializer;

  /** Bind transport and graph mutation owners. */
  constructor(api: LegacyWorkflowAuthoringApi, materializer: NativeWorkflowMaterializer) {
    this.#api = api;
    this.#materializer = materializer;
  }

  /** Reconcile and import one workflow without queueing execution. */
  async import(
    workflow: Record<string, unknown>,
    origin: Vec2,
  ): Promise<NativeWorkflowMaterializationResult> {
    const response = await this.#api.compile(workflow);
    if (!response.plan) throw new Error('Persisted Cube workflow produced no native plan.');
    return this.#materializer.materializeWorkflow(response.plan, origin);
  }
}
