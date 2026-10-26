import { EditorActionsRuntime } from './EditorActionsRuntime.js';
import { editorActionFeedback } from './infrastructure/EditorActionFeedback.js';
import { CanvasDimensionCommands } from './application/CanvasDimensionCommands.js';
import { ComfyDimensionDiscovery } from './infrastructure/ComfyDimensionDiscovery.js';
import { ComfySpatialProbeClient } from './infrastructure/ComfySpatialProbeClient.js';
/** Keep host registration thin while retaining native backend addressing and per-user context. */
export function createEditorActionsRuntime(adapter, catalog, logger, feedback, models, dimension, preparePrompt) {
    const document = adapter.getDocument(), api = adapter.getApi();
    if (!document || !api?.fetchApi)
        throw new Error('Comfy editor actions require a document and API adapter.');
    const fetchApi = api.fetchApi;
    const fetch = (path, options) => fetchApi.call(api, path, options);
    const discovery = new ComfyDimensionDiscovery(adapter.getApp(), preparePrompt, new ComfySpatialProbeClient(fetch, (error) => {
        logger.warn('SugarCubes spatial probe cleanup failed', { error });
    }));
    return new EditorActionsRuntime({
        document,
        app: adapter.getApp(),
        catalog,
        dimensions: new CanvasDimensionCommands(() => dimension.canvases.geometryHost(), discovery),
        workflowContext: () => dimension.sessions.read().active,
        modelContributions: (root) => models.contextContributions(root),
        fetchApi: fetch,
        ...editorActionFeedback(logger, feedback),
    });
}
