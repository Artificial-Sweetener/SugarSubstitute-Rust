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
import { isRecord } from '../types/common.js';
/** Keep presentation lifecycle registration outside the host extension entry point. */
export class ComfyWorkflowWorkspace {
    options;
    canvas;
    masonry;
    #presentation;
    /** Inject existing runtime and surface collaborators without reproducing their policies. */
    constructor(options, masonry, logger) {
        this.options = options;
        this.#presentation = new ComfyWorkflowViewPreferences(options.execution.sessions, () => options.document.defaultView?.localStorage ?? null, () => {
            const app = isRecord(options.app) ? options.app : {};
            const ui = isRecord(app.ui) ? app.ui : {};
            const settings = isRecord(ui.settings) ? ui.settings : {};
            const read = settings.getSettingValue;
            const value = typeof read === 'function'
                ? read.call(settings, 'SugarCubes.OutputCanvas.Visible', true)
                : true;
            return typeof value === 'boolean' ? value : true;
        }, logger);
        this.canvas = new ComfyCanvasHost({ ...options, presentation: this.#presentation });
        this.masonry = new WorkflowMasonryHostLifecycle({
            ...masonry,
            presentation: this.#presentation,
            getContainer: () => this.canvas.content,
        });
    }
    /** Register view restoration before consumers react to workflow activation. */
    setup() {
        this.#presentation.setup();
        this.canvas.setup();
        this.masonry.setup();
        this.options.document.defaultView?.addEventListener('pagehide', this.#pagehide);
    }
    /** Release lightweight session state alongside the presentation surfaces at window teardown. */
    #pagehide = () => {
        this.options.document.defaultView?.removeEventListener('pagehide', this.#pagehide);
        this.#presentation.dispose();
        this.canvas.dispose();
        this.masonry.dispose();
    };
}
