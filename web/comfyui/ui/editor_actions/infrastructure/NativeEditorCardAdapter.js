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
/** Adapt recognized Comfy Vue controls without replacing their value or connection owners. */
import { isRecord } from '../../types/common.js';
import { findVueComponents } from '../../surface/ComfyVueTree.js';
import { copySavablePresetValue } from '../domain/PresetCodec.js';
import { readNativeEditorModelInputs } from './NativeEditorModelInputs.js';
const components = new Map([
    ['WidgetInputNumber', 'number'],
    ['WidgetInputNumberInput', 'number'],
    ['WidgetInputNumberSlider', 'number'],
    ['WidgetToggleSwitch', 'boolean'],
    ['WidgetSelect', 'combo'],
    ['WidgetSelectDefault', 'combo'],
    ['WidgetInputText', 'string'],
    ['WidgetTextarea', 'string'],
]);
/** Resolve a positively identified node component and only its own standard widget descendants. */
export function readNativeEditorCard(root) {
    const mount = projectedEditorCardMount(root);
    if (!mount)
        return null;
    const record = isRecord(mount) ? mount : {};
    const vnode = findVueComponents(record._vnode, 'LGraphNode').find((vnode) => elementOf(vnode) === root);
    const instance = isRecord(vnode?.component) ? vnode.component : {};
    const props = isRecord(instance.props) ? instance.props : {};
    const data = isRecord(props.nodeData) ? props.nodeData : {};
    if (typeof data.type !== 'string')
        return null;
    const fields = [];
    const widgetContainers = findVueComponents(vnode, 'NodeWidgets')
        .map(elementOf)
        .filter((element) => element?.closest('.lg-node') === root);
    for (const [component, kind] of components) {
        for (const child of findVueComponents(vnode, component)) {
            const field = readField(child, root, kind, widgetContainers);
            if (field && !fields.some((existing) => existing.name === field.name))
                fields.push(field);
        }
    }
    const hasOwnedWidgets = Array.isArray(data.sugarcubesEditorWidgets);
    const declared = hasOwnedWidgets ? data.sugarcubesEditorWidgets : data.widgets;
    if (Array.isArray(declared))
        for (const widget of declared) {
            if (!isRecord(widget) || fields.some((field) => field.name === widget.name))
                continue;
            const field = readUnrenderedField(widget, root, hasOwnedWidgets);
            if (field)
                fields.push(field);
        }
    // Preserve authored order regardless of Vue's traversal direction or nested wrapper components.
    const names = Array.isArray(declared)
        ? declared.filter(isRecord).map((widget) => widget.name)
        : [];
    fields.sort((left, right) => names.indexOf(left.name) - names.indexOf(right.name));
    const own = (selector) => [...root.querySelectorAll(selector)].find((element) => element.closest('.lg-node') === root) ?? null;
    const header = own('.lg-node-header');
    return {
        root,
        nodeType: data.type,
        title: typeof data.title === 'string' ? data.title : data.type,
        fields,
        modelInputs: readNativeEditorModelInputs(declared, fields),
        header,
        advanced: readAdvancedControl(data.sugarcubesAdvancedInputs, own('[data-testid="advanced-inputs-button"]')),
    };
}
/** Restrict editor actions to the shared native-card mounts owned by masonry and Cube faces. */
export function projectedEditorCardMount(root) {
    return root.closest('.sugarcubes-cube-face__native-card-mount.sugarcubes-native-node-card');
}
/** Prefer the exact native disclosure owner, retaining Comfy's public button for ordinary graph cards. */
function readAdvancedControl(value, button) {
    if (isRecord(value) &&
        typeof value.shown === 'function' &&
        typeof value.setShown === 'function') {
        const shown = value.shown, setShown = value.setShown;
        return {
            shown: () => shown.call(value) === true,
            setShown: (next) => {
                setShown.call(value, next);
            },
        };
    }
    if (!button)
        return null;
    return {
        shown: () => button.textContent?.includes('Hide') ?? false,
        setShown: (next) => {
            if (next !== (button.textContent?.includes('Hide') ?? false))
                button.click();
        },
    };
}
/** Retain the native emitter and re-read dynamic constraints at every command execution. */
function readField(vnode, root, kind, containers) {
    const instance = isRecord(vnode.component) ? vnode.component : {};
    const props = isRecord(instance.props) ? instance.props : {};
    const widget = isRecord(props.widget) ? props.widget : {};
    const element = elementOf(vnode), emit = instance.emit;
    if (!element ||
        element.closest('.lg-node') !== root ||
        typeof widget.name !== 'string' ||
        widget.name === 'control_after_generate' ||
        typeof emit !== 'function')
        return null;
    const options = () => (isRecord(widget.options) ? widget.options : {});
    const disabled = () => {
        const constraints = options();
        const input = element.querySelector('input,textarea');
        return (!root.isConnected ||
            props.disabled === true ||
            props.readonly === true ||
            constraints.disabled === true ||
            constraints.readonly === true ||
            Boolean(input?.disabled || input?.readOnly));
    };
    const read = () => props.modelValue;
    return {
        name: widget.name,
        kind,
        element: fieldRow(element, containers) ?? element,
        read,
        disabled,
        write: (value) => {
            if (disabled())
                return false;
            const next = normalizeValue(kind, value, options(), widget.spec);
            if (next === undefined)
                return false;
            emit.call(instance, 'update:modelValue', next);
            // The existing promoted-widget lease observes native edit events after Vue callbacks settle.
            element.dispatchEvent(new Event('change', { bubbles: true }));
            return true;
        },
    };
}
/** Apply declared advanced controls through the same data callback used by Comfy's NodeWidgets. */
function readUnrenderedField(widget, root, owned) {
    if (typeof widget.name !== 'string' ||
        widget.name === 'control_after_generate' ||
        (!owned && !Array.isArray(widget.spec) && !isRecord(widget.spec)))
        return null;
    const options = () => (isRecord(widget.options) ? widget.options : {});
    if (options().canvasOnly === true || widget.serialize === false)
        return null;
    const kind = widget.type === 'number'
        ? 'number'
        : widget.type === 'toggle'
            ? 'boolean'
            : widget.type === 'combo'
                ? 'combo'
                : ['text', 'string', 'customtext'].includes(String(widget.type))
                    ? 'string'
                    : owned && typeof widget.type === 'string' && typeof widget.callback === 'function'
                        ? 'json'
                        : null;
    if (!kind)
        return null;
    const disabled = () => !root.isConnected ||
        options().disabled === true ||
        options().readonly === true ||
        (isRecord(widget.slotMetadata) && widget.slotMetadata.linked === true);
    return {
        name: widget.name,
        kind,
        element: root,
        read: () => widget.value,
        disabled,
        write: (value) => {
            if (disabled() || typeof widget.callback !== 'function')
                return false;
            const next = normalizeValue(kind, value, options(), widget.spec);
            if (next === undefined)
                return false;
            widget.value = next;
            widget.callback(next);
            root.dispatchEvent(new Event('change', { bubbles: true }));
            return true;
        },
    };
}
/** Match native numeric setters' clamping while refusing incompatible or malformed values. */
function normalizeValue(kind, value, options, spec) {
    if (kind === 'json')
        return copySavablePresetValue(value);
    if (kind === 'boolean')
        return typeof value === 'boolean' ? value : undefined;
    if (kind === 'string')
        return typeof value === 'string' ? value : undefined;
    if (kind === 'combo')
        return typeof value === 'string' &&
            Array.isArray(options.values) &&
            options.values.includes(value)
            ? value
            : undefined;
    if (typeof value !== 'number' || !Number.isFinite(value))
        return undefined;
    const integer = options.precision === 0 ||
        (isRecord(spec) && spec.type === 'INT') ||
        (Array.isArray(spec) && spec[0] === 'INT');
    if (integer && !Number.isInteger(value))
        return undefined;
    const min = typeof options.min === 'number' ? options.min : -Infinity;
    const max = typeof options.max === 'number' ? options.max : Infinity;
    if (Number.isNaN(min) || Number.isNaN(max) || min > max)
        return undefined;
    const next = Math.min(max, Math.max(min, value));
    return Number.isFinite(next) && (!integer || Number.isInteger(next)) ? next : undefined;
}
/** Include labels and padding only within a positively identified native NodeWidgets row. */
function fieldRow(element, containers) {
    for (const container of containers)
        for (const row of container.children)
            if (row instanceof HTMLElement && row !== element && row.contains(element))
                return row;
    return null;
}
/** Narrow Vue's external DOM root before associating any menu action with it. */
function elementOf(vnode) {
    const instance = isRecord(vnode.component) ? vnode.component : {};
    const tree = isRecord(instance.subTree) ? instance.subTree : {};
    return tree.el instanceof HTMLElement
        ? tree.el
        : vnode.el instanceof HTMLElement
            ? vnode.el
            : null;
}
