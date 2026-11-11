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
/** Compose workflow execution, native fields and optional canvas inputs at their shared host boundary. */
import { createComfyWorkflowExecutionRuntime } from '../execution/infrastructure/ComfyWorkflowExecutionRuntime.js';
import {
  createWorkflowFieldHost,
  type WorkflowFieldHostOptions,
} from './WorkflowFieldComposition.js';
import { createCanvasInputs } from '../input_canvas/infrastructure/CanvasInputComposition.js';
import type { DimensionFieldTarget } from '../editor_actions/application/CanvasDimensionFields.js';
import type { ContextMenuEntry } from '../menus/ContextMenu.js';
import { ComfyWorkflowWorkspace } from '../surface/ComfyWorkflowWorkspace.js';
import type { ComfyCanvasHostOptions } from '../output_canvas/infrastructure/ComfyCanvasHost.js';
import type { WorkflowMasonryHostLifecycleOptions } from '../surface/masonry/WorkflowMasonryHostLifecycle.js';

/** Bind peer canvas capabilities without teaching the web output viewer about native document editing. */
export function createWorkflowCanvasComposition(
  options: Omit<
    WorkflowFieldHostOptions,
    'seeds' | 'activity' | 'progress' | 'connected' | 'subscribeConnection' | 'sessions'
  > & {
    api: unknown;
    logger: Pick<Console, 'error' | 'warn'>;
    /** Show a non-blocking notification through the product's toast owner. */
    notify(severity: 'error' | 'warn', title: string, message: string): void;
  },
): {
  workflowExecution: ReturnType<typeof createComfyWorkflowExecutionRuntime>;
  /** Prompt-scene queueing for the queue bridge and each prompt editor's "Queue this scene". */
  scenes: ReturnType<typeof createComfyWorkflowExecutionRuntime>['scenes'];
  workflowFields: ReturnType<typeof createWorkflowFieldHost>;
  canvasInputs: ReturnType<typeof createCanvasInputs>;
  /** Compose presentation from these same execution, route, settings and page owners. */
  workspace(
    canvas: Pick<ComfyCanvasHostOptions, 'saving' | 'report'>,
    masonry: Omit<
      WorkflowMasonryHostLifecycleOptions,
      'app' | 'document' | 'presentation' | 'getContainer'
    >,
  ): ComfyWorkflowWorkspace;
  /** Canvas geometry and feature contributions for the width/height field menu. */
  editorDimensions: {
    readonly canvases: typeof workflowExecution.canvases;
    readonly sessions: typeof workflowExecution.sessions;
    contributions(targets: readonly DimensionFieldTarget[]): ContextMenuEntry[];
  };
} {
  const workflowExecution = createComfyWorkflowExecutionRuntime({
    app: options.app,
    api: options.api,
    logger: options.logger,
    notify: options.notify,
  });
  const workflowFields = createWorkflowFieldHost({
    ...options,
    sessions: workflowExecution.sessions,
    seeds: workflowExecution.seeds,
    activity: workflowExecution.activity,
    progress: workflowExecution.progress,
    connected: () => workflowExecution.connected,
    subscribeConnection: (listener) => workflowExecution.subscribeConnection(listener),
  });
  const window = options.document.defaultView;
  if (window)
    workflowExecution.bindCanvas(workflowFields.canvas, window, (message) =>
      options.notify('error', 'Canvas delivery failed', message),
    );
  const canvasInputs = createCanvasInputs({
    document: options.document,
    host: (id) => workflowExecution.canvases.sourceHost(id),
    canvases: {
      editable: (id) => {
        const capabilities = workflowExecution.canvases.layerHost(id)?.readCanvas()?.capabilities;
        return Boolean(capabilities?.insertImageLayer && capabilities.positionImageLayer);
      },
      provisions: (id) => workflowExecution.canvases.workflowHost(id) !== null,
    },
    workflowHost: () => workflowExecution.canvases.workflowHost(),
    workflowPeers: () => workflowExecution.peerWorkflowStates(),
    commitWorkflow: () => workflowExecution.commitLoadedWorkflowState(),
    accent: () => workflowFields.appearance.palette.theme.color,
    subscribeAccent: (listener) => workflowFields.appearance.palette.subscribe(listener),
    notice: (title, message) => options.notify('warn', title, message),
    legacyHost: (id) => workflowExecution.canvases.maskHost(id),
    graph: () => options.app?.graph,
    report: options.reportPresentationError,
    logger: options.logger,
    memory: options.memory,
  });
  return {
    workflowExecution,
    scenes: workflowExecution.scenes,
    workflowFields,
    canvasInputs,
    workspace: (canvas, masonry) =>
      new ComfyWorkflowWorkspace(
        {
          ...canvas,
          app: options.app,
          document: options.document,
          memory: options.memory,
          execution: workflowExecution,
          route: canvasInputs.surfaces.route,
          settingsRoute: workflowFields.settingsRoute,
          logger: options.logger,
        },
        { ...masonry, app: options.app, document: options.document },
        options.logger,
      ),
    editorDimensions: {
      canvases: workflowExecution.canvases,
      sessions: workflowExecution.sessions,
      contributions: (targets) => canvasInputs.surfaces.resolutionEntries(targets),
    },
  };
}
