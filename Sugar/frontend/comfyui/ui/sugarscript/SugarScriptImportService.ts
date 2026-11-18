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
/** Coordinate SugarScript compilation and native workflow materialization. */

import type { Vec2 } from '../types/common.js';
import type { SugarScriptDiagnostic } from './SugarScriptAuthoringModels.js';
import type { SugarScriptAuthoringApi } from './SugarScriptAuthoringApi.js';
import type { ExactIntegerField } from '../workflow/ExactIntegerRelations.js';
import { readImportedNodeLinks, type ImportedNodeLink } from '../workflow/ImportedNodeLinks.js';
import {
  readImportedScriptRelations,
  type ImportedScriptRelations,
} from '../workflow/ImportedScriptState.js';
import type {
  NativeWorkflowMaterializationResult,
  NativeWorkflowMaterializer,
} from '../workflow/NativeWorkflowMaterializer.js';

/** Report located compilation failures without collapsing them into plain text. */
export class SugarScriptCompileError extends Error {
  readonly diagnostics: readonly SugarScriptDiagnostic[];

  /** Retain every recovered diagnostic for future presentation surfaces. */
  constructor(diagnostics: readonly SugarScriptDiagnostic[]) {
    const first = diagnostics[0];
    super(
      first
        ? `${first.message} (line ${String(first.span.start.line)})`
        : 'SugarScript is invalid.',
    );
    this.name = 'SugarScriptCompileError';
    this.diagnostics = diagnostics;
  }
}

/** What a script says beyond its Cubes' values, given to the owners that keep it. */
export interface SugarScriptWorkflowState {
  /** Record the digits of whole numbers this page cannot hold. */
  recordExact(fields: readonly ExactIntegerField[]): void;
  /** Give the script's Cubes exactly the node links it records. */
  adoptNodeLinks(instanceIds: readonly string[], links: readonly ImportedNodeLink[]): void;
  /** Give the script's Cubes its overrides, own values and fixed seeds. */
  adoptRelations(instanceIds: readonly string[], script: ImportedScriptRelations): void;
  /** Keep on the workflow what only the script's writer reads. */
  carry(workflow: Record<string, unknown>): void;
}

/** Keep compilation optional and make the resulting native graph self-contained. */
export class SugarScriptImportService {
  readonly #api: SugarScriptAuthoringApi;
  readonly #materializer: NativeWorkflowMaterializer;
  readonly #state: SugarScriptWorkflowState;

  /** Bind transport, graph mutation and the owners of a script's state. */
  constructor(
    api: SugarScriptAuthoringApi,
    materializer: NativeWorkflowMaterializer,
    state: SugarScriptWorkflowState,
  ) {
    this.#api = api;
    this.#materializer = materializer;
    this.#state = state;
  }

  /**
   * Compile and import source without queueing execution.
   *
   * A script records which nodes follow another Cube's and which values every Cube shares.
   * Its Cubes take exactly that once they exist: a node the script does not link is
   * independent, and a value it does not share is each Cube's own.
   */
  async import(source: string, origin: Vec2): Promise<NativeWorkflowMaterializationResult> {
    const response = await this.#api.compile(source);
    if (!response.plan) throw new SugarScriptCompileError(response.diagnostics);
    const result = this.#materializer.materializeSugarScript(response.plan, source, origin);
    const workflow = response.workflow;
    if (workflow) {
      this.#state.carry(workflow);
      this.#state.adoptNodeLinks(result.instanceIds, readImportedNodeLinks(workflow) ?? []);
      this.#state.adoptRelations(result.instanceIds, readImportedScriptRelations(workflow));
    }
    if (response.exactIntegers.length) this.#state.recordExact(response.exactIntegers);
    return result;
  }
}
