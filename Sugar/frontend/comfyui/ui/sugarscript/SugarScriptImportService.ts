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

/** Keep compilation optional and make the resulting native graph self-contained. */
export class SugarScriptImportService {
  readonly #api: SugarScriptAuthoringApi;
  readonly #materializer: NativeWorkflowMaterializer;

  /** Bind transport and graph mutation owners. */
  constructor(api: SugarScriptAuthoringApi, materializer: NativeWorkflowMaterializer) {
    this.#api = api;
    this.#materializer = materializer;
  }

  /** Compile and import source without queueing execution. */
  async import(source: string, origin: Vec2): Promise<NativeWorkflowMaterializationResult> {
    const response = await this.#api.compile(source);
    if (!response.plan) throw new SugarScriptCompileError(response.diagnostics);
    return this.#materializer.materializeSugarScript(response.plan, source, origin);
  }
}
