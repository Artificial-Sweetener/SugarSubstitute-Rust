import { EditorActionsRuntime } from './EditorActionsRuntime.js';
import { editorActionFeedback } from './infrastructure/EditorActionFeedback.js';
/** Keep host registration thin while retaining native backend addressing and per-user context. */
export function createEditorActionsRuntime(adapter, catalog, logger, feedback, models) {
    const document = adapter.getDocument(), api = adapter.getApi();
    if (!document || !api?.fetchApi)
        throw new Error('Comfy editor actions require a document and API adapter.');
    const fetchApi = api.fetchApi;
    return new EditorActionsRuntime({
        document,
        app: adapter.getApp(),
        catalog,
        modelContributions: (root) => models.contextContributions(root),
        fetchApi: (path, options) => fetchApi.call(api, path, options),
        ...editorActionFeedback(logger, feedback),
    });
}
