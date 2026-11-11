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
/** Extend exact native seed popovers while Comfy retains ownership of its generation mode controls. */
import { isRecord } from '../../types/common.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import { findNativeNodeVNode, findVueComponents, resolveVueElement } from '../ComfyVueTree.js';
import { readNativeWidgetMount } from '../NativeWidgetMountBoundary.js';
import { ComfyNativeSeedControlAdapter, } from './ComfyNativeSeedControl.js';
import { createNativeSeedReuseRow } from './NativeSeedReuseRow.js';
/** Attach one restore action to the popup identified by the originating seed button's aria-controls. */
export class NativeSeedMenuHost {
    document;
    reportError;
    #adapter;
    #observer = null;
    #pending = null;
    #popup = null;
    #row = null;
    #closing = false;
    /** Resolve native fields at their explicit interaction boundary, independently of themed styling. */
    constructor(app, document, reuse, reportError) {
        this.document = document;
        this.reportError = reportError;
        this.#adapter = new ComfyNativeSeedControlAdapter(app, reuse);
    }
    /** Observe identified popup surfaces without intercepting Comfy's native mode-button click. */
    setup() {
        if (this.#observer)
            return;
        this.document.addEventListener('click', this.#click, true);
        this.#observer = new HostSurfaceObserver(this.document.body, '[role="dialog"]', (changes) => {
            for (const change of changes) {
                if (change.kind === 'removed' && change.element === this.#popup)
                    this.#retire();
                else if (change.kind !== 'removed')
                    this.#mount(change.element);
            }
        }, { content: true });
    }
    /** Close captured native popovers before graph replacement and retire their restore actions. */
    close() {
        const button = this.#pending?.button;
        const open = this.#popup?.isConnected && button?.getAttribute('aria-expanded') === 'true';
        this.#retire();
        if (open && button?.isConnected) {
            this.#closing = true;
            try {
                button.click();
                button.focus({ preventScroll: true });
            }
            finally {
                this.#closing = false;
            }
        }
    }
    /** Release native input and popup subscriptions at host teardown. */
    dispose() {
        this.close();
        this.#observer?.disconnect();
        this.#observer = null;
        this.document.removeEventListener('click', this.#click, true);
        this.#adapter.endBatch();
    }
    /** Capture only recognized native seed buttons while preserving their original mode and popup behavior. */
    #click = (event) => {
        if (this.#closing || !(event.target instanceof Element))
            return;
        const button = event.target.closest('button[data-testid="value-control"]');
        if (!button || button.disabled || button.closest('[data-sc-seed-box]'))
            return;
        try {
            const control = this.#control(button);
            if (control)
                this.#pending = {
                    button,
                    root: button.closest('.sugarcubes-native-widget,.lg-node'),
                    control,
                };
        }
        catch (error) {
            this.reportError(error);
        }
    };
    /** Append once after the native lazy popup supplies its mode controls; unrelated dialogs stay untouched. */
    #mount(popup) {
        const pending = this.#pending;
        if (!pending ||
            popup.id !== pending.button.getAttribute('aria-controls') ||
            this.#row?.isConnected)
            return;
        const fixed = popup.querySelector('input[type="radio"][value="fixed"]');
        const label = fixed?.closest('label[for="fixed"]');
        if (!fixed ||
            !label?.parentElement ||
            !popup.querySelector('input[type="radio"][value="randomize"]'))
            return;
        const mode = popup.querySelector(`input[type="radio"][value="${pending.control.readMode()}"]`);
        if (mode && !mode.checked)
            mode.click();
        this.#popup = popup;
        this.#row = createNativeSeedReuseRow(this.document, label, pending.control.reuse?.readLast() === null || !pending.control.reuse, () => {
            if (pending !== this.#pending || pending.control.reuse?.readLast() == null)
                return;
            try {
                pending.control.reuse.reuse();
                // WidgetWithControl owns a local mode ref; its original radio updates that ref after the atomic restore.
                if (pending.control.readMode() === 'fixed') {
                    fixed.click();
                    this.close();
                    const trigger = pending.button.isConnected
                        ? pending.button
                        : pending.root?.querySelector('[data-testid="value-control"]');
                    trigger?.focus({ preventScroll: true });
                }
            }
            catch (error) {
                this.reportError(error);
            }
        }, () => this.close());
        label.parentElement.append(this.#row);
    }
    /** Release captured DOM and widget references without changing a removed native popup. */
    #retire() {
        this.#row?.remove();
        this.#row = null;
        this.#popup = null;
        this.#pending = null;
    }
    /** Resolve standalone toolbar mounts directly and graph fields only through their actual Vue component. */
    #control(button) {
        const standalone = button.closest('.sugarcubes-native-widget');
        if (standalone)
            return readNativeWidgetMount(standalone)?.seed ?? null;
        const card = button.closest('.lg-node');
        if (!card)
            return null;
        const vnode = findNativeNodeVNode(card);
        this.#adapter.beginBatch();
        try {
            for (const component of findVueComponents(vnode, [
                'WidgetInputNumber',
                'WidgetInputNumberInput',
                'WidgetInputNumberSlider',
            ])) {
                const owner = isRecord(component.component) ? component.component : {};
                const props = isRecord(owner.props) ? owner.props : {};
                const widget = isRecord(props.widget) ? props.widget : {};
                const element = resolveVueElement(component);
                if (!(element instanceof HTMLElement) || !element.contains(button))
                    continue;
                const options = isRecord(widget.options) ? widget.options : {};
                const precision = typeof options.precision === 'number' ? options.precision : null;
                const control = this.#adapter.read(widget, precision);
                if (control)
                    return control;
            }
            return null;
        }
        finally {
            this.#adapter.endBatch();
        }
    }
}
