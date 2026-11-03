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
/** Translate the established Comfy adapter into explicit editor-action collaborators. */
import type { ComfyAdapter } from '../core/ComfyAdapter.js';
import type { CubeHostFeedback } from '../core/CubeHostFeedback.js';
import type { ModelCatalogClient } from '../model_library/ModelCatalogClient.js';
import type { ModelPickerHost } from '../model_library/ModelPickerHost.js';
import { EditorActionsRuntime, type EditorActionsRuntimeOptions } from './EditorActionsRuntime.js';
import { editorActionFeedback } from './infrastructure/EditorActionFeedback.js';
import type { EditorActionLogger } from './infrastructure/EditorActionFeedback.js';
import { CanvasDimensionCommands } from './application/CanvasDimensionCommands.js';
import { ComfyDimensionDiscovery } from './infrastructure/ComfyDimensionDiscovery.js';
import { ComfySpatialProbeClient } from './infrastructure/ComfySpatialProbeClient.js';
import type { WorkflowCanvasCoordinator } from '../execution/application/WorkflowCanvasCoordinator.js';
import type { WorkflowSessionStore } from '../execution/application/WorkflowSessionStore.js';

/** Keep host registration thin while retaining native backend addressing and per-user context. */
export function createEditorActionsRuntime(
  adapter: Pick<ComfyAdapter, 'getDocument' | 'getApp' | 'getApi'>,
  catalog: ModelCatalogClient,
  logger: EditorActionLogger,
  feedback: CubeHostFeedback,
  models: Pick<ModelPickerHost, 'contextContributions'>,
  dimension: {
    readonly canvases: Pick<WorkflowCanvasCoordinator<Blob>, 'geometryHost'>;
    readonly sessions: Pick<WorkflowSessionStore, 'read'>;
    readonly contributions?: EditorActionsRuntimeOptions['dimensionContributions'];
  },
  preparePrompt: (payload: unknown) => Promise<unknown>,
): EditorActionsRuntime {
  const document = adapter.getDocument(),
    api = adapter.getApi();
  if (!document || !api?.fetchApi)
    throw new Error('Comfy editor actions require a document and API adapter.');
  const fetchApi = api.fetchApi;
  const fetch = (path: string, options?: RequestInit) => fetchApi.call(api, path, options);
  const discovery = new ComfyDimensionDiscovery(
    adapter.getApp(),
    preparePrompt,
    new ComfySpatialProbeClient(fetch, (error) => {
      logger.warn('SugarCubes spatial probe cleanup failed', { error });
    }),
  );
  return new EditorActionsRuntime({
    document,
    app: adapter.getApp(),
    catalog,
    dimensions: new CanvasDimensionCommands(() => dimension.canvases.geometryHost(), discovery),
    workflowContext: () => dimension.sessions.read().active,
    ...(dimension.contributions ? { dimensionContributions: dimension.contributions } : {}),
    modelContributions: (root) => models.contextContributions(root),
    fetchApi: fetch,
    ...editorActionFeedback(logger, feedback),
  });
}
