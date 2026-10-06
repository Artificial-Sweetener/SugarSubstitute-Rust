import { ComfyPromptQueueBridge } from './ComfyPromptQueueBridge.js';
import { CubePromptPipeline } from './CubePromptPipeline.js';
import { CubeOutputPromptAdapter } from './CubeOutputPromptAdapter.js';
import { createCubeDirectExecution } from './CubeDirectExecutionService.js';
import { ComfyCubeOutputEventBridge } from './ComfyCubeOutputEventBridge.js';
import { ComfyCubeOutputHistoryAdapter } from './ComfyCubeOutputHistoryAdapter.js';
import { CubeOutputExecutionStore } from './CubeOutputExecutionStore.js';
/** Preserve existing execution sequencing while adding a common relation boundary for both queue paths. */
export function createCubeHostExecution(options) {
    const outputStore = new CubeOutputExecutionStore();
    const outputEvents = new ComfyCubeOutputEventBridge({
        api: options.api,
        store: outputStore,
        logger: options.logger,
    });
    const outputHistory = new ComfyCubeOutputHistoryAdapter({
        api: options.api,
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
        api: options.api,
        preflight: options.preflight,
        prepare: options.prepareRelations,
        transform: (payload) => pipeline.transform(payload),
        executeDirect: (position, payload, queueOptions) => direct.queue(position, payload, queueOptions),
        shouldExecuteDirect: (payload) => direct.owns(payload),
    });
    return { outputStore, outputEvents, outputHistory, queue };
}
