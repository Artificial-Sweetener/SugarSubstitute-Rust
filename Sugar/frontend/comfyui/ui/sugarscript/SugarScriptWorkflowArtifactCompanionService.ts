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
/** Validate and retain SugarScript paired with an authoritative imported workflow. */

import type {
  SugarScriptAuthoringResponse,
  SugarScriptDiagnostic,
} from './SugarScriptAuthoringModels.js';
import {
  SugarScriptWorkflowCompanion,
  type WorkflowExtraOwner,
} from './SugarScriptWorkflowCompanion.js';

interface SugarScriptArtifactCompiler {
  /** Compile source without mutating the current workflow. */
  compile(source: string): Promise<SugarScriptAuthoringResponse>;
}

export interface RetainedSugarScriptCompanionResult {
  /** Report how the imported source was retained beside workflow authority. */
  state: 'detached' | 'invalid';
  /** Preserve located source diagnostics without failing workflow import. */
  diagnostics: readonly SugarScriptDiagnostic[];
}

/** Own non-executable source validation after Comfy imports the workflow authority. */
export class SugarScriptWorkflowArtifactCompanionService {
  readonly #compiler: SugarScriptArtifactCompiler;
  readonly #getGraph: () => WorkflowExtraOwner | null;

  /** Bind the language transport and late-bound authoritative root workflow. */
  constructor(compiler: SugarScriptArtifactCompiler, getGraph: () => WorkflowExtraOwner | null) {
    this.#compiler = compiler;
    this.#getGraph = getGraph;
  }

  /** Compile and retain paired source without replacing or mutating workflow topology. */
  async retain(source: string): Promise<RetainedSugarScriptCompanionResult> {
    const graph = this.#getGraph();
    if (!graph) throw new Error('Comfy root graph is unavailable.');
    const companion = new SugarScriptWorkflowCompanion(graph);
    let response: SugarScriptAuthoringResponse;
    try {
      response = await this.#compiler.compile(source);
    } catch (error: unknown) {
      companion.attachImported(source, {
        state: 'unverified',
        reason: 'compilation_unavailable',
        diagnostics: [],
      });
      throw error;
    }
    if (response.plan) {
      companion.attachImported(source, {
        state: 'detached',
        reason: 'synchronization_unproven',
        semanticHash: response.plan.semanticHash,
        diagnostics: response.diagnostics,
      });
      return { state: 'detached', diagnostics: response.diagnostics };
    }
    companion.attachImported(source, {
      state: 'invalid',
      reason: 'source_diagnostics',
      diagnostics: response.diagnostics,
    });
    return { state: 'invalid', diagnostics: response.diagnostics };
  }
}
