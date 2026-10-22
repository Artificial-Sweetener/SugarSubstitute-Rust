import { isRecord } from '../types/common.js';
/** Observe programmatic widget replacement while preserving native accessors and callback ownership. */
export class PromptWidgetBinding {
    #node;
    #widget;
    #descriptor;
    #inheritedAccessor;
    #changed;
    #value;
    #writing = false;
    #observing = false;
    /** Capture the actual widget once; renderer projections never become value owners. */
    constructor(node, widget, changed) {
        this.#node = node;
        this.#widget = widget;
        this.#changed = changed;
        this.#descriptor = Object.getOwnPropertyDescriptor(widget, 'value');
        const original = nativeValueDescriptor(widget);
        this.#inheritedAccessor = !this.#descriptor && !!(original?.get || original?.set);
        this.#value = widget.value;
        if (this.#descriptor?.configurable === false)
            return;
        Object.defineProperty(widget, 'value', {
            configurable: true,
            enumerable: original?.enumerable ?? true,
            get: () => (original?.get ? original.get.call(widget) : this.#value),
            set: (value) => {
                if (original?.set)
                    original.set.call(widget, value);
                else {
                    if (original?.get || original?.writable === false)
                        throw new TypeError('The native prompt value is read-only.');
                    this.#value = value;
                }
                const source = widget.value;
                if (!this.#writing && typeof source === 'string')
                    this.#changed(source);
            },
        });
        this.#observing = true;
    }
    /** Read exact serialized text without coercing invalid host values. */
    read() {
        return typeof this.#widget.value === 'string' ? this.#widget.value : '';
    }
    /** Commit one source edit through native callbacks and one balanced workflow history bracket. */
    commit(source) {
        if (this.#writing || source === this.#widget.value)
            return;
        const node = this.#node;
        const widget = this.#widget;
        const root = isRecord(node.rootGraph) ? node.rootGraph : node.graph;
        const before = root?.beforeChange;
        const after = root?.afterChange;
        const transactional = typeof before === 'function' && typeof after === 'function';
        const previous = widget.value;
        this.#writing = true;
        try {
            if (transactional)
                before.call(root);
            try {
                widget.value = source;
                widget.callback?.call(widget, source, node, node.graph?.canvas);
                if (typeof node.onWidgetChanged === 'function')
                    node.onWidgetChanged.call(node, widget.name, source, previous, widget);
                node.graph?.setDirtyCanvas?.(true, true);
            }
            finally {
                if (transactional)
                    after.call(root);
            }
        }
        finally {
            this.#writing = false;
        }
    }
    /** Restore native property ownership while retaining the last committed text. */
    dispose() {
        if (!this.#observing)
            return;
        this.#observing = false;
        const value = this.#widget.value;
        if (this.#descriptor) {
            Object.defineProperty(this.#widget, 'value', this.#descriptor.get || this.#descriptor.set
                ? this.#descriptor
                : { ...this.#descriptor, value });
        }
        else {
            Reflect.deleteProperty(this.#widget, 'value');
            if (!this.#inheritedAccessor && this.#widget.value !== value)
                this.#widget.value = value;
        }
    }
}
/** Locate the host's actual value contract, including accessors implemented on native widget classes. */
function nativeValueDescriptor(widget) {
    let current = widget;
    while (current) {
        const descriptor = Object.getOwnPropertyDescriptor(current, 'value');
        if (descriptor)
            return descriptor;
        const prototype = Object.getPrototypeOf(current);
        current = typeof prototype === 'object' && prototype !== null ? prototype : null;
    }
    return undefined;
}
