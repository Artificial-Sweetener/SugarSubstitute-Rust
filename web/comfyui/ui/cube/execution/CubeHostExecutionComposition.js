import { ComfyPromptQueueBridge } from '../../execution/infrastructure/ComfyPromptQueueBridge.js';
import { ComfyDirectPromptTransport } from '../../execution/infrastructure/ComfyDirectPromptTransport.js';
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
        ...(options.prepareCanvasInputs ? { prepareInputs: options.prepareCanvasInputs } : {}),
    });
    const transport = new ComfyDirectPromptTransport({ api: options.api });
    const queue = new ComfyPromptQueueBridge({
        api: options.api,
        observer: options.observer,
        preflight: options.preflight,
        prepare: options.prepareRelations,
        transform: async (payload) => {
            const candidate = await pipeline.transform(payload);
            return options.prepareCanvasInputs ? await options.prepareCanvasInputs(candidate) : candidate;
        },
        executeDirect: (position, payload, queueOptions, submit) => transport.queue(payload, () => direct.queue(position, payload, queueOptions), submit),
        shouldExecuteDirect: (payload) => direct.owns(payload),
    });
    return queue;
}
