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
/** Own complete browser lifecycle and measurement for Cube prompt textareas. */
const PRESENTATION_RESIZE_EVENT = 'sugarcubes:prompt-presentation-resize';
/** Identify changed hydrated prompt text without asking presentation consumers to observe value properties. */
export const PROMPT_VALUE_CHANGE_EVENT = 'sugarcubes:prompt-value-change';
/** Autosize bound prompt textareas after hydration, input, and width changes. */
export class CubePromptTextareaAutosizer {
    #bindings = new Map();
    #resizeObserver;
    /** Bind browser scheduling and observation to one active document. */
    constructor(documentRef) {
        const windowRef = documentRef.defaultView;
        if (!windowRef)
            throw new Error('Prompt textarea sizing requires a browser window.');
        this.#resizeObserver =
            typeof windowRef.ResizeObserver === 'function'
                ? new windowRef.ResizeObserver((entries) => this.#handleResize(entries))
                : null;
    }
    /** Own one textarea until its renderer or DOM-widget adapter releases it. */
    bind(textarea, onHeightChange = () => undefined) {
        const existing = this.#bindings.get(textarea);
        if (existing) {
            existing.onHeightChange = onHeightChange;
            this.#fit(existing);
            return;
        }
        const binding = {
            textarea,
            onHeightChange,
            onInput: () => this.#fit(binding),
            onPresentationResize: (event) => {
                const phase = event instanceof CustomEvent ? event.detail : null;
                if (phase === 'prepare')
                    this.#prepare(binding);
                else if (phase === 'measure')
                    this.#measure(binding);
                else if (phase === 'apply')
                    this.#apply(binding);
            },
            measuredHeight: 1,
            measuredWidth: null,
            originalStyle: capturePromptTextareaStyle(textarea.style),
            originalValueProperty: Object.getOwnPropertyDescriptor(textarea, 'value'),
            valueAccessors: resolvePromptTextareaValueAccessors(textarea),
            lastHeight: null,
        };
        this.#observeProgrammaticValueChanges(binding);
        this.#bindings.set(textarea, binding);
        textarea.addEventListener('input', binding.onInput);
        textarea.addEventListener(PRESENTATION_RESIZE_EVENT, binding.onPresentationResize);
        this.#resizeObserver?.observe(textarea);
        this.#fit(binding);
    }
    /** Restore one textarea exactly and stop every browser callback it owns. */
    unbind(textarea) {
        const binding = this.#bindings.get(textarea);
        if (!binding)
            return;
        this.#bindings.delete(textarea);
        textarea.removeEventListener('input', binding.onInput);
        textarea.removeEventListener(PRESENTATION_RESIZE_EVENT, binding.onPresentationResize);
        this.#resizeObserver?.unobserve(textarea);
        restorePromptTextareaValueProperty(textarea, binding.originalValueProperty);
        restorePromptTextareaStyle(textarea.style, binding.originalStyle);
    }
    /** Restore every bound textarea and release shared observation. */
    dispose() {
        for (const binding of [...this.#bindings.values()])
            this.unbind(binding.textarea);
        this.#resizeObserver?.disconnect();
    }
    /** Measure after Comfy changes a bound textarea's rendered width. */
    #handleResize(entries) {
        const bindings = entries.flatMap((entry) => {
            const binding = this.#bindings.get(entry.target);
            if (!binding)
                return [];
            const box = entry.borderBoxSize?.[0];
            if (box &&
                Math.round(box.inlineSize) === binding.measuredWidth &&
                Math.abs(box.blockSize - (binding.lastHeight ?? -1)) < 0.5)
                return [];
            return [binding];
        });
        for (const binding of bindings)
            this.#prepare(binding);
        for (const binding of bindings)
            this.#measure(binding);
        for (const binding of bindings)
            this.#apply(binding);
    }
    /** Observe Comfy and Vue assigning hydrated values without browser input events. */
    #observeProgrammaticValueChanges(binding) {
        const { textarea, valueAccessors } = binding;
        Object.defineProperty(textarea, 'value', {
            configurable: true,
            enumerable: valueAccessors.enumerable,
            get: valueAccessors.get,
            set: (value) => {
                const previous = valueAccessors.get();
                valueAccessors.set(value);
                this.#fit(binding);
                if (valueAccessors.get() !== previous)
                    textarea.dispatchEvent(new Event(PROMPT_VALUE_CHANGE_EVENT, { bubbles: true }));
            },
        });
    }
    /** Apply the complete content height and report only material changes. */
    #fit(binding) {
        this.#prepare(binding);
        this.#measure(binding);
        this.#apply(binding);
    }
    /** Collapse all intrinsic rows before the batch reads content geometry. */
    #prepare({ textarea }) {
        // Collapse intrinsic rows before measuring so content and the current theme's minimum own height.
        textarea.style.setProperty('height', '0px', 'important');
        textarea.style.setProperty('overflow-y', 'hidden', 'important');
    }
    /** Read final content geometry without applying intervening height changes. */
    #measure(binding) {
        const { textarea } = binding;
        binding.measuredWidth = textarea.offsetWidth;
        binding.measuredHeight = Math.max(textarea.scrollHeight, textarea.clientHeight, textarea.offsetHeight, 1);
    }
    /** Publish measured heights only after every batch member has been read. */
    #apply(binding) {
        const { textarea, measuredHeight: height } = binding;
        textarea.style.setProperty('height', `${String(height)}px`, 'important');
        if (binding.lastHeight !== null && Math.abs(binding.lastHeight - height) < 0.5)
            return;
        binding.lastHeight = height;
        binding.onHeightChange(height);
    }
}
/** Settle owned prompt sizing after a synchronous presentation-width change. */
export function settleCubePromptTextareaGeometry(roots) {
    const textareas = [
        ...new Set([...roots].flatMap((root) => [...root.querySelectorAll('textarea')])),
    ];
    for (const phase of ['prepare', 'measure', 'apply'])
        for (const textarea of textareas)
            textarea.dispatchEvent(new CustomEvent(PRESENTATION_RESIZE_EVENT, { detail: phase }));
}
/** Capture the exact inline properties owned while a prompt is bound. */
function capturePromptTextareaStyle(style) {
    return {
        height: style.height,
        heightPriority: style.getPropertyPriority('height'),
        overflowY: style.overflowY,
        overflowYPriority: style.getPropertyPriority('overflow-y'),
    };
}
/** Resolve the native value accessors so observation preserves textarea semantics. */
function resolvePromptTextareaValueAccessors(textarea) {
    let owner = textarea;
    while (owner) {
        const descriptor = Object.getOwnPropertyDescriptor(owner, 'value');
        if (typeof descriptor?.get === 'function' && typeof descriptor.set === 'function') {
            return {
                enumerable: descriptor.enumerable ?? false,
                get: () => {
                    const value = descriptor.get?.call(textarea);
                    return typeof value === 'string' ? value : String(value ?? '');
                },
                set: (value) => descriptor.set?.call(textarea, value),
            };
        }
        owner = Reflect.getPrototypeOf(owner);
    }
    throw new Error('Prompt textarea sizing requires native value accessors.');
}
/** Restore the exact value-property ownership that existed before binding. */
function restorePromptTextareaValueProperty(textarea, descriptor) {
    if (descriptor)
        Object.defineProperty(textarea, 'value', descriptor);
    else
        Reflect.deleteProperty(textarea, 'value');
}
/** Restore the original inline style without leaving blank owned declarations. */
function restorePromptTextareaStyle(style, snapshot) {
    restoreStyleProperty(style, 'height', snapshot.height, snapshot.heightPriority);
    restoreStyleProperty(style, 'overflow-y', snapshot.overflowY, snapshot.overflowYPriority);
}
/** Restore one exact inline property and priority. */
function restoreStyleProperty(style, property, value, priority) {
    if (value)
        style.setProperty(property, value, priority);
    else
        style.removeProperty(property);
}
