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
/** Adapt identified native toolbar wrappers to the Substitute row without moving Vue-owned controls. */
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { substituteToolbarCss } from './SubstituteToolbarStyles.js';
const EXTENSIONS_ICON = '[class~="icon-[comfy--extensions-blocks]"]';
/** Mark reversible native frame boundaries without adding graph or widget discovery. */
export class SubstituteToolbarLayout {
    document;
    #presence;
    #style;
    #marked = new Map();
    /** Observe native toolbar controls, including the manager's independently mounted button. */
    constructor(document) {
        this.document = document;
        this.#style = document.createElement('style');
        this.#style.textContent = substituteToolbarCss;
        document.head.append(this.#style);
        this.#presence = new HostSurfaceObserver(document.body, `.actionbar-container,button[aria-label="Workflow actions"],${EXTENSIONS_ICON}`, () => this.#reconcile());
        this.#reconcile();
    }
    /** Restore every native wrapper when Substitute mode or its frame mount is retired. */
    dispose() {
        this.#presence.disconnect();
        for (const [element, attribute] of this.#marked)
            element.removeAttribute(attribute);
        this.#marked.clear();
        this.#style.remove();
    }
    /** Identify the shared native row structurally and release marks on replaced surfaces. */
    #reconcile() {
        const next = new Map();
        const bar = this.document.querySelector('.actionbar-container');
        const workflow = this.document.querySelector('button[aria-label="Workflow actions"]');
        if (workflow?.parentElement)
            next.set(workflow.parentElement, 'data-sc-workflow-selector');
        if (bar) {
            next.set(bar, 'data-sc-substitute-toolbar');
            const extensions = bar.parentElement?.querySelector(EXTENSIONS_ICON)?.closest('button');
            if (extensions?.parentElement && !bar.contains(extensions))
                next.set(extensions.parentElement, 'data-sc-substitute-extensions');
            let line = bar.parentElement;
            while (line && !line.contains(workflow))
                line = line.parentElement;
            if (line && workflow) {
                next.set(line, 'data-sc-substitute-toolbar-line');
                if (line.parentElement?.matches('.sc-output-workspace-topmenu'))
                    next.set(line.parentElement, 'data-sc-substitute-toolbar-menu');
                const workflowColumn = [...line.children].find((child) => child.contains(workflow));
                if (workflowColumn instanceof HTMLElement)
                    next.set(workflowColumn, 'data-sc-substitute-workflow-column');
                let column = bar;
                while (column.parentElement && column.parentElement !== line)
                    column = column.parentElement;
                next.set(column, 'data-sc-substitute-toolbar-column');
            }
        }
        for (const [element, attribute] of this.#marked)
            if (next.get(element) !== attribute)
                element.removeAttribute(attribute);
        this.#marked.clear();
        for (const [element, attribute] of next) {
            if (!element.hasAttribute(attribute))
                element.setAttribute(attribute, '');
            this.#marked.set(element, attribute);
        }
    }
}
