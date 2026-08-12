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
/** Own host graph configuration lifecycle orchestration. */

import type { ComfyApp } from '/scripts/app.js';
import type { SugarCubesUI } from '../SugarCubesUI.js';
import type { CubeHostFeedback } from '../core/CubeHostFeedback.js';
import type { OverlayManager } from '../overlays/OverlayManager.js';
import type { ComfyApplication } from '../types/graph.js';
import type { CubeAffordanceHostLifecycle } from '../affordance/CubeAffordanceHostLifecycle.js';
import type { ComfyCubeRuntime } from './ComfyCubeRuntime.js';
import type { ComfyCubeRuntimeLifecycle } from './ComfyCubeRuntimeLifecycle.js';
import type { CubeWorkflowPreconfiguration } from './CubeWorkflowPreconfiguration.js';
import type { ComfyCubeOutputHistoryAdapter } from './execution/ComfyCubeOutputHistoryAdapter.js';
import type { CubeOutputExecutionStore } from './execution/CubeOutputExecutionStore.js';

interface LifecycleLogger {
  error(...values: unknown[]): void;
}

interface CubeGraphConfigurationLifecycleOptions {
  app: ComfyApplication | null;
  ui: SugarCubesUI;
  runtimeLifecycle: ComfyCubeRuntimeLifecycle;
  requireRuntime: () => ComfyCubeRuntime;
  preconfiguration: CubeWorkflowPreconfiguration;
  affordances: CubeAffordanceHostLifecycle;
  overlays: OverlayManager;
  outputStore: CubeOutputExecutionStore;
  outputHistory: ComfyCubeOutputHistoryAdapter;
  feedback: CubeHostFeedback;
  logger: LifecycleLogger;
}

/** Coordinate graph teardown, hydration, migration, and refresh scheduling. */
export class CubeGraphConfigurationLifecycle {
  constructor(private readonly options: CubeGraphConfigurationLifecycleOptions) {}

  /** Prepare Cube owners before Comfy configures one graph payload. */
  beforeConfigureGraph(graphData?: unknown): void {
    const options = this.options;
    try {
      options.runtimeLifecycle.reset();
      options.outputStore.clear();
      options.preconfiguration.prepare(graphData);
      options.affordances.attach(options.requireRuntime());
      options.overlays.proximity.resetOverlayState();
    } catch (error: unknown) {
      options.logger.error(
        `SugarCubes: Cube preconfiguration failed: ${options.feedback.readErrorMessage(error)}`,
        error,
      );
      throw error;
    }
  }

  /** Restore Cube state after Comfy finishes configuring the active graph. */
  afterConfigureGraph(missingNodeTypes: unknown[], comfyApp: ComfyApp): void {
    const options = this.options;
    try {
      const runtime = options.requireRuntime();
      runtime.hostPlacementGuard.completeHydration();
      runtime.restoreLegacy(options.preconfiguration.takeLegacyBatch());
      runtime.detectLegacyBlueprints();
      const nestedCubes = runtime.graphInventory.snapshot().nestedCubes;
      if (nestedCubes.length) {
        options.feedback.pushToast(
          'error',
          'Nested SugarCubes need attention',
          `${String(nestedCubes.length)} SugarCube${nestedCubes.length === 1 ? '' : 's'} ` +
            `${nestedCubes.length === 1 ? 'is' : 'are'} inside a Subgraph. ` +
            'The workflow was preserved, but execution and Cube saving are blocked until the nested wrapper is removed.',
        );
      }
      options.affordances.adaptMissingNodes(missingNodeTypes);
      void options.outputHistory.hydrateRecent();
      const graph = options.app?.canvas?.graph ?? comfyApp.graph ?? options.app?.graph;
      options.overlays.proximity.refreshOverlayState({
        recompute: true,
        ...(graph ? { graph } : {}),
      });
      options.ui.instanceManager.scheduleRefresh({
        ...(graph ? { graph } : {}),
        reason: 'configure',
      });
      options.ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'configure' });
    } catch (error: unknown) {
      options.logger.error(
        `SugarCubes: Cube postconfiguration failed: ${options.feedback.readErrorMessage(error)}`,
        error,
      );
      throw error;
    }
  }
}
