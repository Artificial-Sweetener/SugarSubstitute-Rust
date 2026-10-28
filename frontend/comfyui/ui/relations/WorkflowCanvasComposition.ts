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

/** Bind peer canvas capabilities without teaching the web output viewer about native document editing. */
export function createWorkflowCanvasComposition(
  options: Omit<
    WorkflowFieldHostOptions,
    'seeds' | 'activity' | 'progress' | 'connected' | 'subscribeConnection' | 'sessions'
  > & {
    api: unknown;
    logger: Pick<Console, 'error' | 'warn'>;
    notifyCanvas(message: string): void;
  },
): {
  workflowExecution: ReturnType<typeof createComfyWorkflowExecutionRuntime>;
  workflowFields: ReturnType<typeof createWorkflowFieldHost>;
  canvasInputs: ReturnType<typeof createCanvasInputs>;
} {
  const workflowExecution = createComfyWorkflowExecutionRuntime({
    app: options.app,
    api: options.api,
    logger: options.logger,
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
  if (window) workflowExecution.bindCanvas(workflowFields.canvas, window, options.notifyCanvas);
  const canvasInputs = createCanvasInputs({
    document: options.document,
    host: (id) => workflowExecution.canvases.sourceHost(id),
    editable: (id) => {
      const capabilities = workflowExecution.canvases.layerHost(id)?.readCanvas()?.capabilities;
      return Boolean(capabilities?.insertImageLayer && capabilities.positionImageLayer);
    },
    legacyHost: (id) => workflowExecution.canvases.maskHost(id),
    graph: () => options.app?.graph,
    report: options.reportPresentationError,
    logger: options.logger,
  });
  return { workflowExecution, workflowFields, canvasInputs };
}
