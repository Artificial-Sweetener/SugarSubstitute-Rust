import { isRecord } from '../types/common.js';
import { nativeNodeFieldRows } from './NativeNodeFieldRows.js';
import { NativeChoiceRestrictions, } from './NativeChoiceRestrictions.js';
import { registerNativePresentationWidget } from './NativePresentationWidgets.js';
import { refreshNativeNodeCards } from './NativeNodeCardRefresh.js';
import { NativeFieldActionMount } from './NativeFieldActionMount.js';
import { NativeChoicePreparation } from './NativeChoicePreparation.js';
/** Keep display-only widgets out of serialization while the native node owns their rendering and theme. */
export class NativeNodeFields {
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
    mountChoice(root, label, change, key = label, prepare) {
        const owner = this.#field(label, key, 'combo');
        const release = this.#retain(owner, (value) => {
            if (typeof value === 'string')
                change(value);
        });
        const name = owner.widget.name;
        let constraints = null, preparation = null, anchor = null, disposed = false;
        return {
            update: (value, choices, disabled = false, visible = true) => {
                if (disposed)
                    return;
                const options = owner.widget.options;
                if (!options)
                    throw new Error('Comfy native field options are unavailable.');
                this.#update(owner, value, [choices, disabled, visible], {
                    hidden: !visible,
                    disabled,
                    values: choices.map((choice) => choice.value),
                    allowedValues: choices
                        .filter((choice) => !choice.blockers.length)
                        .map((choice) => choice.value),
                    getOptionLabel: (value) => choices.find((choice) => choice.value === value)?.label ?? value,
                });
                const row = nativeNodeFieldRows(root).get(name) ?? null;
                if (anchor !== row) {
                    constraints?.dispose();
                    preparation?.dispose();
                    anchor = row;
                    constraints = row ? new NativeChoiceRestrictions(row) : null;
                    preparation = row && prepare ? new NativeChoicePreparation(row, prepare) : null;
                }
                constraints?.update(choices.map((choice) => ({ value: choice.label, blockers: choice.blockers })));
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                constraints?.dispose();
                preparation?.dispose();
                release();
            },
        };
    }
    /** Mount native numeric fields with optional shared spinner-slider presentation. */
    mountNumber(label, change, range, key = label) {
        const owner = this.#field(label, key, 'number');
        const release = this.#retain(owner, (value) => {
            if (typeof value === 'number' && Number.isFinite(value))
                change(value);
        });
        let disposed = false;
        return {
            update: (value, disabled = false, visible = true) => {
                if (!disposed)
                    this.#update(owner, value, [range, disabled, visible], {
                        ...range,
                        // LiteGraph's legacy step is ten times the actual increment; Nodes 2 reads step2.
                        step: range.step * 10,
                        step2: range.step,
                        hidden: !visible,
                        disabled,
                    });
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                release();
            },
        };
    }
    /** Present a read-only text value and shared command inside Comfy's own field row. */
    mountActionField(root, label, actionLabel, action, key = label) {
        const owner = this.#field(label, key, 'string');
        const release = this.#retain(owner, () => { });
        const command = new NativeFieldActionMount(root, actionLabel, action);
        let disposed = false;
        return {
            update: (value, disabled = false, visible = true, title = value) => {
                if (disposed)
                    return;
                this.#update(owner, value, [disabled, visible, title], {
                    read_only: true,
                    title,
                    hidden: !visible,
                    disabled,
                });
                command.update(owner.widget.name, disabled || !visible);
            },
            dispose: () => {
                if (disposed)
                    return;
                disposed = true;
                command.dispose();
                release();
            },
        };
    }
    /** Construct each native field once, preserving the live node's graph-widget array. */
    #field(label, key, type) {
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
                    type,
                    value: type === 'number' ? 0 : '',
                    serialize: false,
                    options: { serialize: false, ...(type === 'combo' ? { values: [] } : {}) },
                    callback: (value) => {
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
        if (field.widget.type !== type)
            throw new Error('The native presentation field type changed.');
        return field;
    }
    /** Share one active commit owner and one native presentation lease across simultaneous views. */
    #retain(owner, change) {
        const field = owner;
        if (!field.callbacks.size)
            field.release = registerNativePresentationWidget(this.node, field.widget, this.beforeField);
        field.callbacks.add(change);
        return () => {
            owner.callbacks.delete(change);
            if (owner.callbacks.size)
                return;
            owner.release();
            owner.signature = '';
            // Comfy retains registered widget state until the graph closes; reuse its exact options identity.
            const options = owner.widget.options;
            if (options && owner.widget.type === 'combo') {
                options.values = [];
                options.allowedValues = [];
                delete options.getOptionLabel;
            }
            owner.widget.value = '';
            owner.widget.hidden = true;
            owner.widget.disabled = true;
            this.#refresh();
        };
    }
    /** Synchronize accepted values without replacing options retained by native widget stores. */
    #update(owner, value, details, options) {
        if (owner.widget.value !== value)
            owner.widget.value = value;
        const signature = JSON.stringify([value, details]);
        if (owner.signature === signature)
            return;
        owner.signature = signature;
        if (!owner.widget.options)
            throw new Error('Comfy native field options are unavailable.');
        Object.assign(owner.widget.options, { serialize: false, ...options });
        owner.widget.hidden = options.hidden;
        owner.widget.disabled = options.disabled;
        this.#refresh();
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
