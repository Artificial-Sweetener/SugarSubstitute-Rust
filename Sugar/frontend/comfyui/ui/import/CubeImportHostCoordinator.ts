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
/** Coordinate native workflow imports and SugarScript host adapters. */

import type { SugarCubesUI } from '../SugarCubesUI.js';
import type { ComfyCubeRuntime } from '../cube/ComfyCubeRuntime.js';
import { CubeHostFeedback } from '../core/CubeHostFeedback.js';
import type { ComfyApplication } from '../types/graph.js';
import type { Vec2 } from '../types/common.js';
import {
  ComfyCanvasDropOriginAdapter,
  type CubePlacementCanvas,
} from './ComfyCanvasDropOriginAdapter.js';
import { SugarScriptAuthoringApi } from '../sugarscript/SugarScriptAuthoringApi.js';
import { SugarScriptFileDropAdapter } from '../sugarscript/SugarScriptFileDropAdapter.js';
import { SugarScriptImportService } from '../sugarscript/SugarScriptImportService.js';
import {
  isSugarScriptFileHost,
  SugarScriptImageFileHostAdapter,
} from '../sugarscript/SugarScriptImageFileHostAdapter.js';
import type { NativeWorkflowMaterializationResult } from '../workflow/NativeWorkflowMaterializer.js';
import { SugarScriptWorkflowArtifactCompanionService } from '../sugarscript/SugarScriptWorkflowArtifactCompanionService.js';
import { NativeWorkflowMaterializer } from '../workflow/NativeWorkflowMaterializer.js';
import { LegacyWorkflowAuthoringApi } from '../workflow/LegacyWorkflowAuthoringApi.js';
import { LegacyWorkflowImportService } from '../workflow/LegacyWorkflowImportService.js';

interface ImportLogger {
  warn(...values: unknown[]): void;
}

interface CubeImportHostCoordinatorOptions {
  ui: SugarCubesUI;
  app: ComfyApplication | null;
  feedback: CubeHostFeedback;
  getRuntime: () => ComfyCubeRuntime;
  logger: ImportLogger;
}

/** Own native workflow import entrypoints and their file adapters. */
export class CubeImportHostCoordinator {
  readonly #ui: SugarCubesUI;
  readonly #app: ComfyApplication | null;
  readonly #feedback: CubeHostFeedback;
  readonly #getRuntime: () => ComfyCubeRuntime;
  readonly #dropOrigin: ComfyCanvasDropOriginAdapter;
  readonly #sugarScriptApi: SugarScriptAuthoringApi;
  readonly #legacyWorkflowApi: LegacyWorkflowAuthoringApi;
  readonly #sugarScriptFiles: SugarScriptFileDropAdapter | null;
  readonly #sugarScriptImages: SugarScriptImageFileHostAdapter | null;

  constructor(options: CubeImportHostCoordinatorOptions) {
    this.#ui = options.ui;
    this.#app = options.app;
    this.#feedback = options.feedback;
    this.#getRuntime = options.getRuntime;
    const { adapter } = this.#ui;
    this.#dropOrigin = new ComfyCanvasDropOriginAdapter({
      getCanvas: () => adapter.getCanvas() as CubePlacementCanvas | null,
      logger: options.logger,
    });
    this.#sugarScriptApi = new SugarScriptAuthoringApi(this.#ui.api);
    this.#legacyWorkflowApi = new LegacyWorkflowAuthoringApi(this.#ui.api);
    const document = adapter.getDocument();
    this.#sugarScriptFiles = document
      ? new SugarScriptFileDropAdapter({
          document,
          importSource: (source) => this.importSugarScript(source),
          feedback: this.#ui.toast,
          readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
        })
      : null;
    this.#sugarScriptFiles?.setup();
    this.#sugarScriptImages = isSugarScriptFileHost(this.#app)
      ? new SugarScriptImageFileHostAdapter({
          host: this.#app,
          importSource: (source) => this.importSugarScript(source),
          importWorkflow: (workflow) => this.importLegacyWorkflow(workflow),
          validateWorkflow: (workflow) => this.#legacyWorkflowApi.compile(workflow),
          retainWorkflowSource: (source) =>
            new SugarScriptWorkflowArtifactCompanionService(
              this.#sugarScriptApi,
              () => this.#app?.graph ?? null,
            ).retain(source),
          feedback: this.#ui.toast,
          readErrorMessage: (error) => this.#feedback.readErrorMessage(error),
        })
      : null;
    this.#sugarScriptImages?.setup();
  }

  /** Return the current graph-space drop origin. */
  computeDropOrigin(): Vec2 {
    return this.#dropOrigin.compute();
  }

  /** Compile source and atomically create a self-contained native Cube workflow. */
  async importSugarScript(
    source: string,
    origin: Vec2 = this.computeDropOrigin(),
  ): Promise<NativeWorkflowMaterializationResult> {
    const runtime = this.#getRuntime();
    runtime.graphScope.assertCurrentRoot('SugarScript import');
    const graph = this.#app?.graph;
    if (!graph) throw new Error('Comfy root graph is unavailable.');
    return new SugarScriptImportService(
      this.#sugarScriptApi,
      this.#nativeWorkflowMaterializer(runtime, graph),
    ).import(source, origin);
  }

  /** Reconcile and atomically import a persisted group-era Cube workflow. */
  async importLegacyWorkflow(
    workflow: Record<string, unknown>,
    origin: Vec2 = this.computeDropOrigin(),
  ): Promise<NativeWorkflowMaterializationResult> {
    const runtime = this.#getRuntime();
    runtime.graphScope.assertCurrentRoot('persisted Cube workflow import');
    const graph = this.#app?.graph;
    if (!graph) throw new Error('Comfy root graph is unavailable.');
    return new LegacyWorkflowImportService(
      this.#legacyWorkflowApi,
      this.#nativeWorkflowMaterializer(runtime, graph),
    ).import(workflow, origin);
  }

  /** Build the shared native workflow mutation owner for one root graph. */
  #nativeWorkflowMaterializer(
    runtime: ComfyCubeRuntime,
    graph: NonNullable<ComfyApplication['graph']>,
  ): NativeWorkflowMaterializer {
    return new NativeWorkflowMaterializer(runtime.placement, graph, {
      register: (payload) => runtime.registerSubgraphs(payload),
      discard: (ids) => runtime.discardSubgraphs(ids),
    });
  }
}
