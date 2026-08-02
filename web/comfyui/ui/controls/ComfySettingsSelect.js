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
/** Mount Comfy's Settings selects in SugarCubes-owned metadata surfaces. */
import { isRecord } from '../types/common.js';
import { loadComfySettingsAutoCompleteComponent, loadComfySettingsSelectComponent, loadComfyVueRenderRuntime, } from '../surface/ComfyRuntimeModuleLoader.js';
import { findComfyVueAppContext } from '../surface/ComfyVueTree.js';
/** Own native Settings component discovery and every resulting Vue mount. */
export class InstalledComfySettingsSelectRenderer {
    #document;
    #runtime = null;
    /** Bind installed component discovery to the active Comfy document. */
    constructor(documentRef) {
        this.#document = documentRef;
    }
    /** Mount Comfy's Settings SingleSelect with its body-level popover. */
    mountSingle(target, props) {
        return this.#mount(target, props, ({ appContext, select, vue }, currentProps) => {
            const vnode = vue.h(select, {
                'aria-label': currentProps.ariaLabel,
                appendTo: 'body',
                class: 'sugarcubes-comfy-settings-select',
                disabled: currentProps.disabled ?? false,
                modelValue: currentProps.value,
                'onUpdate:modelValue': currentProps.onChange,
                optionLabel: 'label',
                optionValue: 'value',
                options: currentProps.options,
            });
            return { appContext, vnode, vue };
        });
    }
    /** Mount Comfy's editable Settings AutoComplete with body-level suggestions. */
    mountAutocomplete(target, props) {
        return this.#mount(target, props, ({ appContext, autoComplete, vue }, currentProps) => {
            const vnode = vue.h(autoComplete, {
                'aria-label': currentProps.ariaLabel,
                appendTo: 'body',
                autoOptionFocus: true,
                class: 'sugarcubes-comfy-settings-autocomplete',
                delay: 0,
                disabled: currentProps.disabled ?? false,
                forceSelection: false,
                modelValue: currentProps.values,
                multiple: true,
                'onUpdate:modelValue': (values) => {
                    currentProps.onChange(readAutocompleteValues(values));
                },
                onComplete: (event) => {
                    currentProps.onComplete(readAutocompleteQuery(event));
                },
                placeholder: currentProps.placeholder ?? '',
                suggestions: currentProps.suggestions,
            });
            return { appContext, vnode, vue };
        });
    }
    /** Mount one installed control and provide a race-safe unmount handle. */
    #mount(target, initialProps, createVNode) {
        let disposed = false;
        let props = initialProps;
        let revision = 0;
        const root = this.#document.getElementById('vue-app');
        if (!root || !findComfyVueAppContext(root)) {
            target.dataset.comfySettingsControlState = 'unavailable';
            return { update: () => undefined, unmount: () => undefined };
        }
        const render = () => {
            const requestedRevision = ++revision;
            void this.#loadRuntime()
                .then((runtime) => {
                if (disposed || requestedRevision !== revision)
                    return;
                const { appContext, vnode, vue } = createVNode(runtime, props);
                if (!isRecord(vnode)) {
                    throw new TypeError('Comfy Settings select returned an invalid VNode.');
                }
                vnode.appContext = appContext;
                vue.render(vnode, target);
            })
                .catch((error) => {
                if (disposed || requestedRevision !== revision)
                    return;
                target.dataset.comfySettingsControlState = 'unavailable';
                console.error('[SugarCubes] Unable to mount Comfy Settings select.', error);
            });
        };
        render();
        return {
            update: (nextProps) => {
                if (disposed)
                    return;
                props = nextProps;
                render();
            },
            unmount: () => {
                if (disposed)
                    return;
                disposed = true;
                revision += 1;
                void this.#runtime
                    ?.then(({ vue }) => {
                    vue.render(null, target);
                })
                    .catch(() => undefined);
            },
        };
    }
    /** Resolve both Settings controls, the Vue renderer, and Comfy's app context once. */
    async #loadRuntime() {
        this.#runtime ??= Promise.all([
            loadComfySettingsAutoCompleteComponent(this.#document),
            loadComfySettingsSelectComponent(this.#document),
            loadComfyVueRenderRuntime(this.#document),
        ]).then(([autoComplete, select, vue]) => {
            const root = this.#document.getElementById('vue-app');
            const appContext = root ? findComfyVueAppContext(root) : null;
            if (!appContext) {
                throw new Error('Comfy Vue application context was not found.');
            }
            return { appContext, autoComplete, select, vue };
        });
        return await this.#runtime;
    }
}
/** Narrow AutoComplete's host value before it reaches application state. */
function readAutocompleteValues(value) {
    if (!Array.isArray(value))
        return [];
    return value.filter((entry) => typeof entry === 'string');
}
/** Narrow PrimeVue's completion event before filtering application suggestions. */
function readAutocompleteQuery(event) {
    return isRecord(event) && typeof event.query === 'string' ? event.query : '';
}
/** Own the state and lifecycle of one Comfy Settings SingleSelect. */
export class ComfySettingsSingleSelectControl {
    element;
    #renderer;
    #ariaLabel;
    #onChange;
    #disabled;
    #mount = null;
    #options;
    #value;
    /** Create one single-value Settings select. */
    constructor(documentRef, renderer, props) {
        this.element = documentRef.createElement('div');
        this.element.className = 'sugarcubes-comfy-settings-select-host';
        this.#renderer = renderer;
        this.#ariaLabel = props.ariaLabel;
        this.#disabled = props.disabled ?? false;
        this.#onChange = props.onChange;
        this.#options = [...props.options];
        this.#value = props.value;
        this.#render();
    }
    /** Return the selected application value. */
    value() {
        return this.#value;
    }
    /** Replace selectable options and the selected value in one native remount. */
    update({ disabled = this.#disabled, options = this.#options, value = this.#value, }) {
        this.#disabled = disabled;
        this.#options = [...options];
        this.#value = value;
        this.#render();
    }
    /** Release Comfy's Vue subtree. */
    dispose() {
        this.#mount?.unmount();
        this.#mount = null;
    }
    /** Reconcile the native component with application-owned state. */
    #render() {
        const props = {
            ariaLabel: this.#ariaLabel,
            disabled: this.#disabled,
            options: this.#options,
            value: this.#value,
            onChange: (value) => {
                this.#value = value;
                this.#render();
                this.#onChange(value);
            },
        };
        if (this.#mount) {
            this.#mount.update(props);
            return;
        }
        this.#mount = this.#renderer.mountSingle(this.element, props);
    }
}
/** Own one editable Comfy Settings AutoComplete and its suggestion policy. */
export class ComfySettingsAutocompleteControl {
    element;
    #renderer;
    #ariaLabel;
    #onChange;
    #placeholder;
    #mount = null;
    #options;
    #suggestions = [];
    #values;
    /** Create one editable multi-value Settings autocomplete. */
    constructor(documentRef, renderer, props) {
        this.element = documentRef.createElement('div');
        this.element.className = 'sugarcubes-comfy-settings-autocomplete-host';
        this.#renderer = renderer;
        this.#ariaLabel = props.ariaLabel;
        this.#onChange = props.onChange;
        this.#options = [...props.options];
        this.#placeholder = props.placeholder;
        this.#values = [...props.values];
        this.#render();
    }
    /** Return every selected application value. */
    values() {
        return [...this.#values];
    }
    /** Replace available and selected values in one native remount. */
    update({ options = this.#options, values = this.#values, }) {
        this.#options = [...options];
        this.#values = [...values];
        this.#render();
    }
    /** Release Comfy's Vue subtree. */
    dispose() {
        this.#mount?.unmount();
        this.#mount = null;
    }
    /** Reconcile the native component with application-owned state. */
    #render() {
        const props = {
            ariaLabel: this.#ariaLabel,
            ...(this.#placeholder ? { placeholder: this.#placeholder } : {}),
            suggestions: this.#suggestions,
            values: this.#values,
            onComplete: (query) => {
                const trimmedQuery = query.trim();
                const normalizedQuery = trimmedQuery.toLocaleLowerCase();
                const selected = new Set(this.#values.map((value) => value.toLocaleLowerCase()));
                const matchingOptions = this.#options.filter((option) => !selected.has(option.toLocaleLowerCase()) &&
                    (!normalizedQuery || option.toLocaleLowerCase().includes(normalizedQuery)));
                this.#suggestions =
                    trimmedQuery &&
                        !selected.has(normalizedQuery) &&
                        !matchingOptions.some((option) => option.toLocaleLowerCase() === normalizedQuery)
                        ? [trimmedQuery, ...matchingOptions]
                        : matchingOptions;
                this.#render();
            },
            onChange: (values) => {
                this.#values = uniqueNonEmptyValues(values);
                this.#render();
                this.#onChange(this.#values);
            },
        };
        if (this.#mount) {
            this.#mount.update(props);
            return;
        }
        this.#mount = this.#renderer.mountAutocomplete(this.element, props);
    }
}
/** Normalize editable tags without preventing user-authored model names. */
function uniqueNonEmptyValues(values) {
    const seen = new Set();
    const normalized = [];
    for (const value of values) {
        const trimmed = value.trim();
        const key = trimmed.toLocaleLowerCase();
        if (!trimmed || seen.has(key))
            continue;
        seen.add(key);
        normalized.push(trimmed);
    }
    return normalized;
}
