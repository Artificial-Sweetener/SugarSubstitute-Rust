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
/** Compose the sole Comfy queue and output-event integration without expanding host registration. */
import type { ComfyGraph } from '../../types/graph.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import type { NativeSubgraphBoundaryResolver } from '../graph/NativeSubgraphBoundaryResolver.js';
import { ComfyPromptQueueBridge } from './ComfyPromptQueueBridge.js';
import type { ComfyPromptQueueApi } from './ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './CubeOutputPromptAdapter.js';
import { createCubeDirectExecution } from './CubeDirectExecutionService.js';
import { ComfyCubeOutputEventBridge } from './ComfyCubeOutputEventBridge.js';
import type { ComfyExecutionEventApi } from './ComfyCubeOutputEventBridge.js';
import { ComfyCubeOutputHistoryAdapter } from './ComfyCubeOutputHistoryAdapter.js';
import type { ComfyCubeOutputHistoryApi } from './ComfyCubeOutputHistoryAdapter.js';
import { CubeOutputExecutionStore } from './CubeOutputExecutionStore.js';

export interface CubeHostExecutionOptions {
  api: unknown;
  logger: Pick<Console, 'debug' | 'warn' | 'error'>;
  boundaryResolver: NativeSubgraphBoundaryResolver;
  getGraph(): ComfyGraph | null | undefined;
  getCubes(): readonly CubeNode[];
  getProximityMatches(): readonly unknown[];
  applyProximity(payload: unknown): unknown;
  prepareRelations(payload: unknown): unknown;
  preflight(): void;
}

/** Preserve existing execution sequencing while adding a common relation boundary for both queue paths. */
export function createCubeHostExecution(options: CubeHostExecutionOptions): {
  outputStore: CubeOutputExecutionStore;
  outputEvents: ComfyCubeOutputEventBridge;
  outputHistory: ComfyCubeOutputHistoryAdapter;
  queue: ComfyPromptQueueBridge;
} {
  const outputStore = new CubeOutputExecutionStore();
  const outputEvents = new ComfyCubeOutputEventBridge({
    api: options.api as ComfyExecutionEventApi,
    store: outputStore,
    logger: options.logger,
  });
  const outputHistory = new ComfyCubeOutputHistoryAdapter({
    api: options.api as ComfyCubeOutputHistoryApi,
    store: outputStore,
    logger: options.logger,
  });
  const outputAdapter = new CubeOutputPromptAdapter({
    getCubes: options.getCubes,
    boundaryResolver: options.boundaryResolver,
  });
  const pipeline = new CubePromptPipeline({
    applyProximity: options.applyProximity,
    adaptCubeOutputs: (payload) => outputAdapter.apply(payload),
  });
  const direct = createCubeDirectExecution({
    api: options.api,
    logger: options.logger,
    getGraph: options.getGraph,
    getProximityMatches: options.getProximityMatches,
  });
  const queue = new ComfyPromptQueueBridge({
    api: options.api as ComfyPromptQueueApi,
    preflight: options.preflight,
    prepare: options.prepareRelations,
    transform: (payload) => pipeline.transform(payload),
    executeDirect: (position, payload, queueOptions) =>
      direct.queue(position, payload, queueOptions),
    shouldExecuteDirect: (payload) => direct.owns(payload),
  });
  return { outputStore, outputEvents, outputHistory, queue };
}
