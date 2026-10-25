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
import { ComfyPromptQueueBridge } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import { ComfyDirectPromptTransport } from '../../execution/infrastructure/ComfyDirectPromptTransport.js';
import type { ComfyPromptQueueApi } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import type { PromptQueueObserver } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './CubeOutputPromptAdapter.js';
import { createCubeDirectExecution } from './CubeDirectExecutionService.js';
import type { WorkflowPromptPreparation } from '../../execution/domain/WorkflowPreparation.js';

export interface CubeHostExecutionOptions {
  api: unknown;
  observer: PromptQueueObserver;
  logger: Pick<Console, 'debug' | 'warn' | 'error'>;
  boundaryResolver: NativeSubgraphBoundaryResolver;
  getGraph(): ComfyGraph | null | undefined;
  getCubes(): readonly CubeNode[];
  getProximityMatches(): readonly unknown[];
  applyProximity(payload: unknown): unknown;
  prepareRelations(payload: unknown): unknown;
  preflight(): void;
  prepareCanvasInputs?(payload: unknown): Promise<WorkflowPromptPreparation>;
}

/** Preserve existing execution sequencing while adding a common relation boundary for both queue paths. */
export function createCubeHostExecution(options: CubeHostExecutionOptions): ComfyPromptQueueBridge {
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
    ...(options.prepareCanvasInputs ? { prepareInputs: options.prepareCanvasInputs } : {}),
  });
  const transport = new ComfyDirectPromptTransport({ api: options.api });
  const queue = new ComfyPromptQueueBridge({
    api: options.api as ComfyPromptQueueApi,
    observer: options.observer,
    preflight: options.preflight,
    prepare: options.prepareRelations,
    transform: async (payload) => {
      const candidate = await pipeline.transform(payload);
      return options.prepareCanvasInputs
        ? await options.prepareCanvasInputs(candidate)
        : { payload: candidate };
    },
    executeDirect: (position, payload, queueOptions, submit, prepared) =>
      transport.queue(
        payload,
        () => direct.queue(position, payload, queueOptions, prepared),
        submit,
      ),
    shouldExecuteDirect: (payload) => direct.owns(payload),
  });
  return queue;
}
