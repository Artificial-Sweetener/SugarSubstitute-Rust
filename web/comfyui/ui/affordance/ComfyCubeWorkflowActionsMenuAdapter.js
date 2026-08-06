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
/** Adapt Comfy's portalled Workflow actions menu at the root of a Cube editor. */
/** Own Cube language on the top-level Workflow actions menu. */
export class ComfyCubeWorkflowActionsMenuAdapter {
    #document;
    #canvas;
    #contexts;
    #policy;
    #observer;
    #textStates = new Map();
    #scheduled = false;
    /** Bind the stable semantic menu surface and shared Cube policy. */
    constructor(options) {
        this.#document = options.document;
        this.#canvas = options.canvas;
        this.#contexts = options.contexts;
        this.#policy = options.policy;
        this.#observer = new MutationObserver(() => this.#schedule());
    }
    /** Observe portalled menu mounts and apply Cube language when appropriate. */
    install() {
        this.#observer.observe(this.#document.body, { childList: true, subtree: true });
        this.refresh();
    }
    /** Release the observer and restore any still-mounted host labels. */
    dispose() {
        this.#observer.disconnect();
        this.#restoreText();
    }
    /** Reconcile the currently open Workflow actions menu. */
    refresh() {
        this.#scheduled = false;
        this.#forgetDetachedText();
        if (!this.#contexts.resolveEditor(this.#currentGraph())?.isCubeRoot) {
            this.#restoreText();
            return;
        }
        const menu = this.#activeWorkflowActionsMenu();
        if (!menu)
            return;
        const primarySave = this.#menuItem(menu, '.pi-save', 0);
        const clear = this.#menuItem(menu, '.pi-trash', 0);
        if (primarySave)
            this.#setItemText(primarySave, this.#label('save-workflow'));
        if (clear)
            this.#setItemText(clear, this.#label('clear-container'));
    }
    /** Coalesce Reka mount and text-render mutation bursts. */
    #schedule() {
        if (this.#scheduled)
            return;
        this.#scheduled = true;
        queueMicrotask(() => this.refresh());
    }
    /** Find the workflow dropdown by its unique stable semantic action set. */
    #activeWorkflowActionsMenu() {
        const menus = this.#document.querySelectorAll('[role="menu"][data-reka-menu-content][data-state="open"]');
        return ([...menus].find((menu) => this.#menuItem(menu, '.pi-save', 0) !== null &&
            this.#menuItem(menu, '.pi-download', 0) !== null &&
            this.#menuItem(menu, '.pi-trash', 0) !== null &&
            this.#menuItem(menu, '.pi-times', 0) !== null) ?? null);
    }
    /** Resolve one direct Reka workflow action by semantic icon and occurrence. */
    #menuItem(menu, iconSelector, index) {
        const items = [...menu.querySelectorAll('[role="menuitem"]')].filter((item) => item.querySelector(iconSelector) !== null);
        return items[index] ?? null;
    }
    /** Replace one literal-safe menu label while retaining the host value. */
    #setItemText(item, value) {
        const text = item.querySelector('span.flex-1');
        if (!text)
            return;
        if (!this.#textStates.has(text))
            this.#textStates.set(text, text.textContent ?? '');
        if (text.textContent !== value)
            text.textContent = value;
    }
    /** Restore every connected menu label exactly. */
    #restoreText() {
        for (const [element, value] of this.#textStates) {
            if (element.isConnected)
                element.textContent = value;
        }
        this.#textStates.clear();
    }
    /** Drop records for Reka menu nodes already unmounted by the host. */
    #forgetDetachedText() {
        for (const element of this.#textStates.keys()) {
            if (!element.isConnected)
                this.#textStates.delete(element);
        }
    }
    /** Resolve a label from the renderer-independent policy. */
    #label(actionId) {
        return this.#policy.decide(actionId, this.#contexts.resolveSelection(this.#canvas.selectedItems), this.#contexts.resolveEditor(this.#currentGraph())).label;
    }
    /** Read the graph currently visible in either renderer. */
    #currentGraph() {
        return this.#canvas.subgraph ?? this.#canvas.graph ?? null;
    }
}
