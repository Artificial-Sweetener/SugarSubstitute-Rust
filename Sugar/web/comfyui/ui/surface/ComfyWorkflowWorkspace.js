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
/** Compose workflow view persistence and presentation adapters around one committed session boundary. */
import { ComfyCanvasHost, } from '../output_canvas/infrastructure/ComfyCanvasHost.js';
import { WorkflowMasonryHostLifecycle, } from '../surface/masonry/WorkflowMasonryHostLifecycle.js';
import { ComfyWorkflowViewPreferences } from '../workflow/infrastructure/ComfyWorkflowViewPreferences.js';
import { ComfyWorkflowChangeTracker } from '../workflow/infrastructure/ComfyWorkflowChangeTracker.js';
import { workflowOpensInMasonry } from '../workflow/infrastructure/ComfyWorkflowViewDocument.js';
import { OUTPUT_CANVAS_VISIBLE_SETTING } from '../output_canvas/infrastructure/ComfyCanvasPreferences.js';
import { WORKFLOW_MASONRY_DEFAULT_EDITOR_WIDTH } from './masonry/WorkflowMasonryLayout.js';
import { isRecord } from '../types/common.js';
import { WorkspaceConsole } from '../console/WorkspaceConsole.js';
import { ComfyDesktopSessionHost } from '../workflow/infrastructure/ComfyDesktopSessionHost.js';
import { HOST_CAPABILITIES } from '../core/embedded/PanelRuntime.js';
/** Keep presentation lifecycle registration outside the host extension entry point. */
export class ComfyWorkflowWorkspace {
    options;
    canvas;
    masonry;
    console;
    /** The one owner of SugarCubes' contact with Comfy's workflow change tracker. */
    changeTracker;
    #presentation;
    #desktop;
    /** Inject existing runtime and surface collaborators without reproducing their policies. */
    constructor(options, masonry, logger) {
        this.options = options;
        const view = options.document.defaultView;
        if (!view)
            throw new Error('The workflow workspace needs a browser window.');
        this.changeTracker = new ComfyWorkflowChangeTracker(options.app, view, options.report);
        this.#presentation = new ComfyWorkflowViewPreferences(options.execution.sessions, () => options.document.defaultView?.localStorage ?? null, () => {
            const app = isRecord(options.app) ? options.app : {};
            const ui = isRecord(app.ui) ? app.ui : {};
            const settings = isRecord(ui.settings) ? ui.settings : {};
            const read = settings.getSettingValue;
            const value = typeof read === 'function'
                ? read.call(settings, OUTPUT_CANVAS_VISIBLE_SETTING, true)
                : true;
            return typeof value === 'boolean' ? value : true;
        }, {
            opensInMasonry: (sessionId) => workflowOpensInMasonry(committedDocument(options.execution.workflowDocuments.readHostWorkflow(sessionId))),
            recordView: (view) => this.changeTracker.recordView(view),
        }, logger);
        this.canvas = new ComfyCanvasHost({
            ...options,
            presentation: this.#presentation,
            defaultEditorWidth: WORKFLOW_MASONRY_DEFAULT_EDITOR_WIDTH,
        });
        this.masonry = new WorkflowMasonryHostLifecycle({
            ...masonry,
            presentation: this.#presentation,
            getContainer: () => this.canvas.content,
        });
        this.console = new WorkspaceConsole({
            document: options.document,
            app: options.app,
            getHost: () => this.canvas.content,
            reserve: (height) => this.canvas.reserveBottom(height),
            report: options.report,
        });
        const window = options.document.defaultView;
        this.#desktop = window
            ? new ComfyDesktopSessionHost(window, {
                app: options.app,
                execution: options.execution,
                canvas: this.canvas.sessionSnapshots,
                presentation: this.#presentation,
                ...(options.route ? { route: options.route } : {}),
                report: options.reportSession,
                ...(options.settingsRoute ? { settingsRoute: options.settingsRoute } : {}),
                hostTakesScroll: () => options.host.read()?.declares(HOST_CAPABILITIES.checkpointScroll) === true,
            })
            : null;
    }
    /** Register view restoration before consumers react to workflow activation. */
    setup() {
        this.#presentation.setup();
        this.canvas.setup();
        this.masonry.setup();
        this.console.setup();
        try {
            this.#desktop?.setup();
        }
        catch (error) {
            this.options.reportSession(error, 'protocol');
        }
        this.options.document.defaultView?.addEventListener('pagehide', this.#pagehide);
    }
    /** Release lightweight session state alongside the presentation surfaces at window teardown. */
    #pagehide = () => {
        this.options.document.defaultView?.removeEventListener('pagehide', this.#pagehide);
        this.#desktop?.dispose();
        this.#presentation.dispose();
        this.canvas.dispose();
        this.masonry.dispose();
        this.console.dispose();
    };
}
/** The committed serialized graph of one host workflow: its tracked state, or its saved content. */
function committedDocument(workflow) {
    if (!isRecord(workflow))
        return null;
    if (isRecord(workflow.changeTracker) && isRecord(workflow.changeTracker.activeState))
        return workflow.changeTracker.activeState;
    if (typeof workflow.content !== 'string' || !workflow.content)
        return null;
    try {
        return JSON.parse(workflow.content);
    }
    catch {
        return null;
    }
}
