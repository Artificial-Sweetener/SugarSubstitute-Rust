import { isRecord } from '../types/common.js';
import { nativeNodeFieldRows } from './NativeNodeFieldRows.js';
import { NativeChoiceRestrictions, } from './NativeChoiceRestrictions.js';
import { registerNativePresentationWidget } from './NativePresentationWidgets.js';
import { refreshNativeNodeCards } from './NativeNodeCardRefresh.js';
/** Keep display-only widgets out of serialization while the native node owns their rendering and theme. */
export class NativeNodeChoiceFields {
    node;
    namespace;
    document;
    beforeField;
    #fields = new Map();
    #pending = false;
    /** Scope widget identities and optional placement before an existing native field. */
    constructor(node, namespace, document, beforeField = null) {
        this.node = node;
        this.namespace = namespace;
        this.document = document;
        this.beforeField = beforeField;
    }
    /** Share a native field across simultaneous projections without multiplying value commits. */
    mountChoice(root, label, change, key = label) {
        const name = '__sugarcubes_presentation:' + this.namespace + ':' + key;
        let field = this.#fields.get(name);
        if (!field) {
            const callbacks = new Set();
            const add = this.node.addCustomWidget;
            if (typeof add !== 'function')
                throw new Error('Comfy native node fields are unavailable.');
            const original = this.node.widgets, items = [...(original ?? [])];
            let widget;
            try {
                widget = add.call(this.node, {
                    name,
                    label,
                    type: 'combo',
                    value: '',
                    serialize: false,
                    options: { serialize: false, values: [] },
                    callback: (value) => {
                        if (typeof value === 'string')
                            callbacks.values().next().value?.(value);
                    },
                });
            }
            finally {
                if (original)
                    original.splice(0, original.length, ...items);
                if (original)
                    this.node.widgets = original;
                else
                    delete this.node.widgets;
            }
            if (!isNativeWidget(widget))
                throw new Error('Comfy returned an invalid native field.');
            widget.serialize = false;
            widget.label = label;
            field = {
                widget,
                callbacks,
                signature: '',
                release: () => { },
            };
            this.#fields.set(name, field);
        }
        if (!field.callbacks.size)
            field.release = registerNativePresentationWidget(this.node, field.widget, this.beforeField);
        field.callbacks.add(change);
        const owner = field;
        let constraints = null, anchor = null, disposed = false;
        return {
            update: (value, choices, disabled = false, visible = true) => {
                if (disposed)
                    return;
                // Native controls update optimistically; rejected intents must restore the binding's accepted value.
                if (owner.widget.value !== value)
                    owner.widget.value = value;
                const signature = JSON.stringify([value, choices, disabled, visible]);
                if (owner.signature !== signature) {
                    owner.signature = signature;
                    const options = owner.widget.options;
                    if (!options)
                        throw new Error('Comfy native field options are unavailable.');
                    // Native WidgetValueStore retains this options object when the widget registers.
                    Object.assign(options, {
                        serialize: false,
                        hidden: !visible,
                        disabled,
                        values: choices.map((choice) => choice.value),
                        allowedValues: choices
                            .filter((choice) => !choice.blockers.length)
                            .map((choice) => choice.value),
                        getOptionLabel: (value) => choices.find((choice) => choice.value === value)?.label ?? value,
                    });
                    owner.widget.hidden = !visible;
                    owner.widget.disabled = disabled;
                    this.#refresh();
                }
                const row = nativeNodeFieldRows(root).get(name) ?? null;
                if (anchor !== row) {
                    constraints?.dispose();
                    anchor = row;
                    constraints = row ? new NativeChoiceRestrictions(row) : null;
                }
                constraints?.update(choices.map((choice) => ({ value: choice.label, blockers: choice.blockers })));
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                constraints?.dispose();
                owner.callbacks.delete(change);
                if (owner.callbacks.size)
                    return;
                owner.release();
                owner.signature = '';
                // Comfy retains registered widget state until the graph closes; reuse its exact options identity.
                const options = owner.widget.options;
                if (options) {
                    options.values = [];
                    options.allowedValues = [];
                    delete options.getOptionLabel;
                }
                owner.widget.value = '';
                owner.widget.hidden = true;
                owner.widget.disabled = true;
                this.#refresh();
            },
        };
    }
    /** Coalesce paired source/layer changes into one scoped native card refresh. */
    #refresh() {
        if (this.#pending)
            return;
        this.#pending = true;
        queueMicrotask(() => {
            this.#pending = false;
            refreshNativeNodeCards(this.document, { kind: 'nodes', nodes: [this.node] });
        });
    }
}
/** Admit the native widget without replacing its reactive identity or property descriptors. */
function isNativeWidget(value) {
    return isRecord(value) && typeof value.name === 'string';
}
