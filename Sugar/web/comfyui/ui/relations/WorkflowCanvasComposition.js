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
import { createWorkflowFieldHost, } from './WorkflowFieldComposition.js';
import { createCanvasInputs } from '../input_canvas/infrastructure/CanvasInputComposition.js';
import { attachSelfUpdateWhenUnhosted, comfySelfUpdateTransport, } from '../core/updates/SelfUpdateSource.js';
import { ComfyWorkflowWorkspace } from '../surface/ComfyWorkflowWorkspace.js';
/** Bind peer canvas capabilities without teaching the web output viewer about native document editing. */
export function createWorkflowCanvasComposition(options) {
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
    const selfUpdate = comfySelfUpdateTransport(options.api);
    // SugarCubes updates itself only on a page whose host does not; the page lives as long as this.
    if (selfUpdate)
        attachSelfUpdateWhenUnhosted({
            host: workflowFields.host,
            updates: workflowFields.updates,
            transport: selfUpdate,
            report: (error) => options.logger.warn('SugarCubes update request failed', { error }),
        });
    const window = options.document.defaultView;
    if (window)
        workflowExecution.bindCanvas(workflowFields.canvas, window, (message) => options.notify('error', 'Canvas delivery failed', message));
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
        workspace: (canvas, masonry) => new ComfyWorkflowWorkspace({
            ...canvas,
            app: options.app,
            document: options.document,
            memory: options.memory,
            execution: workflowExecution,
            route: canvasInputs.surfaces.route,
            host: workflowFields.host,
            settingsRoute: workflowFields.settingsRoute,
            logger: options.logger,
        }, { ...masonry, app: options.app, document: options.document }, options.logger),
        editorDimensions: {
            canvases: workflowExecution.canvases,
            sessions: workflowExecution.sessions,
            contributions: (targets) => canvasInputs.surfaces.resolutionEntries(targets),
        },
    };
}
