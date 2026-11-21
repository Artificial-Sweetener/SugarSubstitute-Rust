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
import type { ExactIntegerField } from './ExactIntegerRelations.js';
import { readImportedNodeLinks, type ImportedNodeLink } from './ImportedNodeLinks.js';

/** Keep legacy workflow loading behind exact-version reconciliation. */
export class LegacyWorkflowImportService {
  readonly #api: LegacyWorkflowAuthoringApi;
  readonly #materializer: NativeWorkflowMaterializer;
  readonly #recordExact: (fields: readonly ExactIntegerField[]) => void;
  readonly #adoptLinks: (
    instanceIds: readonly string[],
    links: readonly ImportedNodeLink[],
  ) => void;

  /** Bind transport, graph mutation and the workflow's record of exact whole numbers. */
  constructor(
    api: LegacyWorkflowAuthoringApi,
    materializer: NativeWorkflowMaterializer,
    recordExact: (fields: readonly ExactIntegerField[]) => void,
    adoptLinks: (instanceIds: readonly string[], links: readonly ImportedNodeLink[]) => void,
  ) {
    this.#api = api;
    this.#materializer = materializer;
    this.#recordExact = recordExact;
    this.#adoptLinks = adoptLinks;
  }

  /**
   * Reconcile and import one workflow without queueing execution. Digits of whole numbers this
   * page cannot hold are recorded on the workflow only once its Cubes exist.
   */
  async import(
    workflow: Record<string, unknown>,
    origin: Vec2,
    source?: string,
  ): Promise<NativeWorkflowMaterializationResult> {
    return this.#materialize(await this.#api.compile(workflow, source), origin, workflow);
  }

  /**
   * Open one workflow from a file as a workflow of its own. It is reconciled first, so a file
   * that cannot be imported opens nothing; `place` then opens the workflow that will hold its
   * Cubes and returns where they go. The Cubes keep the identities the file gave them, which a
   * workflow already open may also hold, so they never join it.
   */
  async open(
    workflow: Record<string, unknown>,
    source: string | undefined,
    place: () => Promise<Vec2>,
  ): Promise<NativeWorkflowMaterializationResult> {
    const response = await this.#api.compile(workflow, source);
    if (!response.plan) throw new Error('Persisted Cube workflow produced no native plan.');
    return this.#materialize(response, await place(), workflow);
  }

  /** Create the reconciled workflow's Cubes and record what only they can hold. */
  #materialize(
    response: Awaited<ReturnType<LegacyWorkflowAuthoringApi['compile']>>,
    origin: Vec2,
    workflow: Record<string, unknown>,
  ): NativeWorkflowMaterializationResult {
    if (!response.plan) throw new Error('Persisted Cube workflow produced no native plan.');
    const result = this.#materializer.materializeWorkflow(response.plan, origin);
    // A workflow that records its own links keeps exactly those; its Cubes are not linked for it.
    const links = readImportedNodeLinks(workflow);
    if (links) this.#adoptLinks(result.instanceIds, links);
    // Cube documents record no seed mode, so their seeds take ComfyUI's own control default, as
    // the Python application's did. The recorded digits are kept exactly below, so the first run
    // after import still reproduces the image.
    if (response.exactIntegers.length) this.#recordExact(response.exactIntegers);
    return result;
  }
}
