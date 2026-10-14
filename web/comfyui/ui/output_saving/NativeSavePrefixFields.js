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
/** Lease standard Save Image filename controls while their file naming is managed by SugarCubes. */
import { HostSurfaceObserver } from '../core/HostSurfaceObserver.js';
import { isRecord } from '../types/common.js';
const PREFIX_SELECTOR = '[data-node-id] input[aria-label="filename_prefix"],[data-node-id] textarea[aria-label="filename_prefix"]';
/** Preserve native values, layout and prior control state without recording presentation in history. */
export class NativeSavePrefixFields {
    document;
    #widgets = new Map();
    #inputs = new Map();
    #observer;
    #ids = new Set();
    #disposed = false;
    /** Observe only identified native filename fields; unrelated popups never request graph discovery. */
    constructor(document) {
        this.document = document;
        this.#observer = new HostSurfaceObserver(document.body, PREFIX_SELECTOR, (changes) => {
            for (const change of changes)
                if (change.element instanceof HTMLInputElement ||
                    change.element instanceof HTMLTextAreaElement)
                    this.#field(change.element);
            this.#observer.discardPending();
        }, { attributes: ['readonly'], exclude: '.sugarcubes-source-saving-dialog' });
    }
    /** Apply accepted management state to cached standard-saver handles without changing their widget values. */
    reconcile(sources, managed) {
        if (this.#disposed)
            return;
        const active = new Set();
        this.#ids = new Set();
        if (managed)
            for (const source of sources)
                if (source.node.type === 'SaveImage') {
                    this.#ids.add(String(source.node.id));
                    const widgets = Array.isArray(source.node.widgets) ? source.node.widgets : [];
                    for (const widget of widgets)
                        if (isRecord(widget) && widget.name === 'filename_prefix') {
                            active.add(widget);
                            if (!this.#widgets.has(widget))
                                this.#widgets.set(widget, widget.disabled);
                            if (widget.disabled !== true)
                                widget.disabled = true;
                        }
                }
        for (const [widget, prior] of this.#widgets)
            if (!active.has(widget)) {
                if (widget.disabled === true) {
                    if (prior === undefined)
                        delete widget.disabled;
                    else
                        widget.disabled = prior;
                }
                this.#widgets.delete(widget);
            }
        for (const input of this.document.querySelectorAll(PREFIX_SELECTOR))
            if (input instanceof HTMLInputElement || input instanceof HTMLTextAreaElement)
                this.#field(input);
        for (const input of [...this.#inputs.keys()])
            if (!input.isConnected)
                this.#release(input);
        this.#observer.discardPending();
    }
    /** Restore leased control state and release exactly this feature's observation. */
    dispose() {
        if (this.#disposed)
            return;
        this.reconcile([], false);
        this.#disposed = true;
        this.#observer.disconnect();
        for (const input of [...this.#inputs.keys()])
            this.#release(input);
    }
    /** Reconcile one named field without altering focus, text, attributes on unrelated cards or popup identity. */
    #field(input) {
        const id = input.closest('[data-node-id]')?.getAttribute('data-node-id');
        if (input.isConnected && id && this.#ids.has(id)) {
            if (!this.#inputs.has(input))
                this.#inputs.set(input, input.readOnly);
            if (!input.readOnly)
                input.readOnly = true;
        }
        else
            this.#release(input);
    }
    /** Restore only the read-only property owned by this lease. */
    #release(input) {
        const previous = this.#inputs.get(input);
        if (previous === undefined)
            return;
        if (input.readOnly)
            input.readOnly = previous;
        this.#inputs.delete(input);
    }
}
