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
/** Mount shared values and link selectors through Comfy's exact widget component family. */
import { isRecord } from '../../types/common.js';
import { NativeLinkLabelLayout } from './NativeLinkLabelLayout.js';
import { NativeLinkChoiceLabels } from './NativeLinkChoiceLabels.js';
import { scopeNativeVueMountContext } from '../../surface/NativeVueMountContext.js';
import { findComfyVueAppContext } from '../../surface/ComfyVueTree.js';
import { loadComfyNativeWidgetRuntime } from '../../surface/ComfyNativeWidgetRuntime.js';
import { registerNativeWidgetMount, releaseNativeWidgetMount, } from '../../surface/NativeWidgetMountBoundary.js';
import { NativeChoiceRestrictions } from '../../surface/NativeChoiceRestrictions.js';
import { choiceRestrictions } from '../domain/FieldCompatibility.js';
/** Delegate component rendering and menu interaction to Comfy while relations own only value policy. */
export class NativeRelationWidgetRenderer {
    document;
    reportError;
    #labelLayout;
    #runtime = null;
    /** Bind the host; resolve its native capabilities when the first surface mounts. */
    constructor(document, reportError) {
        this.document = document;
        this.reportError = reportError;
        this.#labelLayout = new NativeLinkLabelLayout(document, reportError);
    }
    /** Mount one native field backed by an automatically discovered shared value. */
    mount(root, initial, change, changeMode) {
        let group = initial;
        let groupSignature = JSON.stringify(initial);
        const seed = initial.members[0]?.semantic === 'seed'
            ? {
                modes: ['fixed', 'randomize', 'increment', 'decrement'],
                readMode: () => group.seedMode,
                writeMode: changeMode,
            }
            : null;
        const mount = this.#mount(root, () => {
            const domain = group.domain;
            const choices = choiceRestrictions(group.members);
            const options = domain.kind === 'number'
                ? {
                    min: domain.minimum,
                    max: domain.maximum,
                    step2: domain.step,
                    ...(domain.integer ? { precision: 0 } : {}),
                }
                : domain.kind === 'choice'
                    ? { values: choices.map((choice) => choice.value) }
                    : {};
            const widget = {
                name: `Global ${group.label}`,
                label: group.label,
                value: group.value,
                type: domain.kind === 'number'
                    ? 'number'
                    : domain.kind === 'choice'
                        ? 'combo'
                        : domain.kind === 'boolean'
                            ? 'toggle'
                            : 'text',
                options,
                ...(seed
                    ? {
                        controlWidget: {
                            value: group.seedMode,
                            update: (mode) => {
                                if (mode === 'fixed' ||
                                    mode === 'randomize' ||
                                    mode === 'increment' ||
                                    mode === 'decrement')
                                    changeMode(mode);
                            },
                        },
                    }
                    : {}),
            };
            return {
                component: domain.kind === 'number'
                    ? 'WidgetInputNumber'
                    : domain.kind === 'choice'
                        ? 'WidgetSelect'
                        : domain.kind === 'boolean'
                            ? 'WidgetToggleSwitch'
                            : 'WidgetInputText',
                widget,
                value: group.value,
                seed,
                choices,
            };
        }, change);
        return {
            update: (next) => {
                const signature = JSON.stringify(next);
                group = next;
                if (signature === groupSignature)
                    return;
                groupSignature = signature;
                mount.refresh();
            },
            dispose: mount.dispose,
        };
    }
    /** Render link and literal choices inside the same native select used by ordinary fields. */
    mountChoice(root, label, change) {
        let value = '';
        let choices = [];
        let disabled = false;
        let signature = '';
        root.dataset.scChoiceInline = '';
        const labels = new NativeLinkChoiceLabels(root, this.#labelLayout);
        const mount = this.#mount(root, () => ({
            component: 'WidgetSelect',
            value,
            seed: null,
            widget: {
                name: label,
                label,
                type: 'combo',
                value,
                options: {
                    disabled,
                    values: choices.map((choice) => choice.value),
                    getOptionLabel: (value) => choices.find((choice) => choice.value === value)?.label ?? value,
                },
            },
            choices: choices.map((choice) => ({ value: choice.label, blockers: choice.blockers })),
        }), (value) => {
            if (typeof value === 'string')
                change(value);
        });
        return {
            update: (next, options, inactive = false, preferredLabels) => {
                const nextSignature = JSON.stringify([next, options, inactive, preferredLabels]);
                if (signature === nextSignature)
                    return;
                signature = nextSignature;
                value = next;
                choices = options;
                disabled = inactive;
                mount.refresh();
                labels.update(preferredLabels ?? options.map((choice) => choice.label));
            },
            dispose: () => {
                labels.dispose();
                delete root.dataset.scChoiceInline;
                mount.dispose();
            },
        };
    }
    /** Own native Vue mounting once for toolbar fields, linked fields, and titlebar selectors. */
    #mount(root, request, change) {
        root.classList.add('sugarcubes-native-widget');
        const retainInteraction = (event) => event.stopPropagation();
        const events = ['pointerdown', 'mousedown', 'click', 'contextmenu'];
        for (const name of events)
            root.addEventListener(name, retainInteraction);
        let disposed = false;
        let render = null;
        let unmount = null;
        const constraints = new NativeChoiceRestrictions(root);
        this.#runtime ??= loadComfyNativeWidgetRuntime(this.document);
        void this.#runtime
            .then((runtime) => {
            if (disposed)
                return;
            const appRoot = root.ownerDocument.getElementById('vue-app');
            const context = appRoot ? findComfyVueAppContext(appRoot) : null;
            if (!context)
                throw new Error('Comfy widget application context is unavailable.');
            const scopedContext = scopeNativeVueMountContext(context);
            render = () => {
                if (disposed)
                    return;
                const { component, widget, value, seed, choices } = request();
                const vnode = runtime.h(runtime.components.get(component), {
                    widget,
                    modelValue: value,
                    'onUpdate:modelValue': (next) => {
                        if (typeof next === 'string' || typeof next === 'number' || typeof next === 'boolean')
                            change(next);
                    },
                });
                if (!isRecord(vnode))
                    throw new TypeError('Comfy returned an invalid widget VNode.');
                vnode.appContext = scopedContext;
                runtime.render(vnode, root);
                registerNativeWidgetMount(root, { vnode, seed });
                constraints.update(choices);
            };
            unmount = () => runtime.render(null, root);
            render();
        })
            .catch(this.reportError);
        return {
            refresh: () => render?.(),
            dispose: () => {
                disposed = true;
                constraints.dispose();
                releaseNativeWidgetMount(root);
                for (const name of events)
                    root.removeEventListener(name, retainInteraction);
                unmount?.();
                root.classList.remove('sugarcubes-native-widget');
            },
        };
    }
}
