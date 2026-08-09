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
/** Bridge Nodes 1 prompt textarea height into renderer-neutral widget measurement. */
import { CubePromptTextareaAutosizer } from './CubePromptTextareaAutosizer.js';
import { clearCubePromptWidgetHeight, setCubePromptWidgetHeight, } from './CubePromptWidgetHeightStore.js';
const DEFAULT_WIDGET_MARGIN = 10;
/** Own graph-widget associations for Nodes 1 prompt DOM elements. */
export class CubePromptDomWidgetSizingHost {
    #autosizer;
    #onGeometryChange;
    #widgets = new Map();
    /** Bind widget allocation updates to one document and geometry owner. */
    constructor(documentRef, onGeometryChange) {
        this.#autosizer = new CubePromptTextareaAutosizer(documentRef);
        this.#onGeometryChange = onGeometryChange;
    }
    /** Reconcile one mounted element with its current semantic prompt widget. */
    reconcile(element, widget) {
        const textarea = asTextarea(element);
        if (!textarea || !widget) {
            this.release(element);
            return;
        }
        const existing = this.#widgets.get(textarea);
        if (existing === widget)
            return;
        if (existing)
            this.release(textarea);
        this.#widgets.set(textarea, widget);
        this.#autosizer.bind(textarea, (contentHeight) => {
            const margin = finiteNonNegative(widget.margin) ?? DEFAULT_WIDGET_MARGIN;
            if (setCubePromptWidgetHeight(widget, contentHeight + margin * 2)) {
                this.#onGeometryChange();
            }
        });
    }
    /** Release sizing owned for one mounted element. */
    release(element) {
        const textarea = asTextarea(element);
        if (!textarea)
            return;
        const widget = this.#widgets.get(textarea);
        if (!widget)
            return;
        this.#autosizer.unbind(textarea);
        clearCubePromptWidgetHeight(widget);
        this.#widgets.delete(textarea);
    }
    /** Restore all graph widgets and browser resources. */
    dispose() {
        for (const textarea of [...this.#widgets.keys()])
            this.release(textarea);
        this.#autosizer.dispose();
    }
}
/** Narrow one mounted element to its active-document textarea type. */
function asTextarea(element) {
    const textareaType = element.ownerDocument.defaultView?.HTMLTextAreaElement;
    return textareaType && element instanceof textareaType ? element : null;
}
/** Accept only finite non-negative widget margins. */
function finiteNonNegative(value) {
    return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : null;
}
