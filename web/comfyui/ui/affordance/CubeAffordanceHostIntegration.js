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
/** Compose focused Cube affordance adapters around one graph-bound runtime. */
import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { isRecord } from '../types/common.js';
import { CubeAffordancePolicy } from './CubeAffordancePolicy.js';
import { ComfyCubeCommandAdapter } from './ComfyCubeCommandAdapter.js';
import { ComfyCubeEditorChromeAdapter } from './ComfyCubeEditorChromeAdapter.js';
import { ComfyCubeNativeMenuAdapter } from './ComfyCubeNativeMenuAdapter.js';
import { ComfyCubeSelectionSurfaceAdapter } from './ComfyCubeSelectionSurfaceAdapter.js';
import { ComfyCubeWorkflowActionsMenuAdapter } from './ComfyCubeWorkflowActionsMenuAdapter.js';
import { CubeMissingNodeLabelAdapter } from './CubeMissingNodeLabelAdapter.js';
import { CubeInstanceRenameGuard } from './CubeInstanceRenameGuard.js';
import { CubeNodeTooltipAdapter } from './CubeNodeTooltipAdapter.js';
/** Keep lifecycle composition separate from each renderer or action responsibility. */
export class CubeAffordanceHostIntegration {
    #document;
    #canvas;
    #controller;
    #logger;
    #policy = new CubeAffordancePolicy();
    #runtime = null;
    #nativeMenus = null;
    #commands = null;
    #selectionSurface = null;
    #editorChrome = null;
    #workflowActions = null;
    #missingNodes = null;
    #instanceRename = null;
    #nodeTooltips = null;
    /** Bind stable host surfaces; graph-specific collaborators arrive through attach. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = requireAffordanceCanvas(options.canvas);
        this.#controller = options.controller;
        this.#logger = options.logger;
    }
    /** Rebind adapters after Comfy replaces the root workflow graph. */
    attach(runtime) {
        if (this.#runtime === runtime)
            return;
        this.#disposeAdapters();
        this.#runtime = runtime;
        this.#missingNodes = new CubeMissingNodeLabelAdapter(runtime.nodes);
        try {
            this.#nativeMenus = new ComfyCubeNativeMenuAdapter(this.#canvas);
            this.#nativeMenus.install();
            this.#commands = new ComfyCubeCommandAdapter({
                document: this.#document,
                canvas: this.#canvas,
                contexts: runtime.contexts,
                policy: this.#policy,
                controller: this.#controller,
            });
            this.#commands.install();
        }
        catch (error) {
            this.#logger.error('SugarCubes: Comfy command/menu compatibility contract failed.', error);
            this.#commands?.dispose();
            this.#commands = null;
        }
        this.#selectionSurface = new ComfyCubeSelectionSurfaceAdapter({
            document: this.#document,
            canvas: this.#canvas,
            contexts: runtime.contexts,
        });
        this.#selectionSurface.install();
        this.#instanceRename = new CubeInstanceRenameGuard({
            document: this.#document,
            canvas: this.#canvas,
            nodes: runtime.nodes,
            contexts: runtime.contexts,
        });
        this.#instanceRename.install();
        this.#nodeTooltips = new CubeNodeTooltipAdapter({
            document: this.#document,
            nodes: runtime.nodes,
        });
        this.#nodeTooltips.install();
        this.#editorChrome = new ComfyCubeEditorChromeAdapter({
            document: this.#document,
            canvas: this.#canvas,
            contexts: runtime.contexts,
            policy: this.#policy,
            controller: this.#controller,
        });
        this.#editorChrome.install();
        this.#workflowActions = new ComfyCubeWorkflowActionsMenuAdapter({
            document: this.#document,
            canvas: this.#canvas,
            contexts: runtime.contexts,
            policy: this.#policy,
        });
        this.#workflowActions.install();
    }
    /** Supply Cube replacements through Comfy's supported additive node-menu hook. */
    getNodeMenuItems(node) {
        if (!isCubeNode(node))
            return [];
        return [
            {
                content: 'Save Cube',
                callback: () => this.#controller.saveCube(node),
            },
        ];
    }
    /** Adapt missing-node hints before Comfy transfers them into warning stores. */
    adaptMissingNodes(missingNodes) {
        return this.#missingNodes?.adapt(missingNodes) ?? 0;
    }
    /** Reconcile Vue surfaces after selection changes not accompanied by a DOM mount. */
    refresh() {
        this.#selectionSurface?.refresh();
        this.#editorChrome?.refresh();
        this.#workflowActions?.refresh();
    }
    /** Release all host mutations owned by this extension instance. */
    dispose() {
        this.#disposeAdapters();
        this.#runtime = null;
    }
    /** Dispose in reverse dependency order before attaching a replacement runtime. */
    #disposeAdapters() {
        this.#workflowActions?.dispose();
        this.#editorChrome?.dispose();
        this.#nodeTooltips?.dispose();
        this.#instanceRename?.dispose();
        this.#selectionSurface?.dispose();
        this.#commands?.dispose();
        this.#nativeMenus?.dispose();
        this.#editorChrome = null;
        this.#workflowActions = null;
        this.#selectionSurface = null;
        this.#instanceRename = null;
        this.#nodeTooltips = null;
        this.#commands = null;
        this.#nativeMenus = null;
        this.#missingNodes = null;
    }
}
/** Validate the smallest stable canvas surface shared by both Comfy renderers. */
function requireAffordanceCanvas(value) {
    if (!isRecord(value) ||
        !(value.selectedItems instanceof Set) ||
        typeof value.getNodeMenuOptions !== 'function' ||
        typeof value.getCanvasMenuOptions !== 'function') {
        throw new TypeError('Comfy Cube affordance canvas integration is unavailable.');
    }
    return value;
}
