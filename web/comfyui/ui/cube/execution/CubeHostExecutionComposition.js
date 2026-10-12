import { ComfyPromptQueueBridge } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './CubeOutputPromptAdapter.js';
import { createCubeDirectExecution } from './CubeDirectExecutionService.js';
/** Preserve existing execution sequencing while adding a common relation boundary for both queue paths. */
export function createCubeHostExecution(options) {
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
        api: options.api,
        observer: options.observer,
        preflight: options.preflight,
        prepare: options.prepareRelations,
        transform: (payload) => pipeline.transform(payload),
        executeDirect: (position, payload, queueOptions) => direct.queue(position, payload, queueOptions),
        shouldExecuteDirect: (payload) => direct.owns(payload),
    });
    return queue;
}
