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
/** Adapt native Comfy dropdowns through their own reactive value and callback path. */
import { isRecord } from '../types/common.js';
import { ComfyNodeVNodeIndex, findVueComponents } from '../surface/ComfyVueTree.js';
import { readNativeWidgetOrigins } from '../surface/NativeWidgetOrigins.js';
import { recognizeModelField } from './ModelFieldRecognition.js';
import { readNativeWidgetMount } from '../surface/NativeWidgetMountBoundary.js';
/**
 * Return only native fields mounted within the requested graph or projected node card.
 *
 * Callers reading many roots in one batch share `nodes` so the application tree is indexed once.
 */
export function readComfyModelFields(root, snapshot, nodes = new ComfyNodeVNodeIndex()) {
    const nodeVNode = nodes.resolve(root);
    if (!nodeVNode || elementOf(nodeVNode) !== root)
        return [];
    const component = isRecord(nodeVNode.component) ? nodeVNode.component : {};
    const nodeProps = isRecord(component.props) ? component.props : {};
    const nodeData = isRecord(nodeProps.nodeData) ? nodeProps.nodeData : {};
    const classType = typeof nodeData.type === 'string' ? nodeData.type : '';
    const fields = [];
    const selects = new Set(findVueComponents(nodeVNode, 'WidgetSelectDefault'));
    for (const widgetRoot of root.querySelectorAll('.sugarcubes-native-widget')) {
        if (widgetRoot.closest('.lg-node') !== root)
            continue;
        const mounted = readNativeWidgetMount(widgetRoot);
        if (mounted)
            for (const vnode of findVueComponents(mounted.vnode, 'WidgetSelectDefault'))
                selects.add(vnode);
    }
    for (const vnode of selects) {
        const binding = readField(vnode, root, classType, snapshot, nodeData.widgets);
        if (binding)
            fields.push(binding);
    }
    return fields;
}
/** Validate native component identity before hiding or replacing any visible control. */
function readField(vnode, root, classType, snapshot, projectedWidgets) {
    const component = isRecord(vnode.component) ? vnode.component : {};
    const props = isRecord(component.props) ? component.props : {};
    const widget = isRecord(props.widget) ? props.widget : {};
    const options = isRecord(widget.options) ? widget.options : {};
    const native = elementOf(vnode);
    const trigger = native?.querySelector('[data-testid="widget-select-default-trigger"]');
    const anchor = trigger?.closest('[data-capture-wheel]');
    if (!anchor || anchor.closest('.lg-node') !== root || typeof widget.name !== 'string')
        return null;
    if (!Array.isArray(options.values) ||
        options.values.some((value) => typeof value !== 'string'))
        return null;
    const readValues = () => {
        const current = isRecord(widget.options) ? widget.options : {};
        const values = Array.isArray(current.values) &&
            !current.values.some((value) => typeof value !== 'string')
            ? current.values.filter((value) => typeof value === 'string')
            : [];
        if (!Object.prototype.hasOwnProperty.call(current, 'allowedValues'))
            return values;
        const allowed = current.allowedValues;
        return Array.isArray(allowed) && allowed.every((value) => typeof value === 'string')
            ? values.filter((value) => allowed.includes(value))
            : [];
    };
    const values = readValues();
    const projectedWidget = Array.isArray(projectedWidgets)
        ? projectedWidgets.find((value) => isRecord(value) && value.name === widget.name)
        : undefined;
    const kind = recognizeModelField(snapshot, classType, widget.name, values, readNativeWidgetOrigins(isRecord(projectedWidget) ? projectedWidget : {}));
    const emit = component.emit;
    const proxy = isRecord(component.proxy) ? component.proxy : {};
    const watch = proxy.$watch;
    const unsetValue = props.modelValue === null || props.modelValue === undefined;
    if (!kind ||
        (typeof props.modelValue !== 'string' && !unsetValue) ||
        typeof emit !== 'function' ||
        typeof watch !== 'function')
        return null;
    return {
        anchor,
        kind,
        get options() {
            return readValues();
        },
        inputName: widget.name,
        ...(typeof widget.nodeLocatorId === 'string' ? { nodeLocator: widget.nodeLocatorId } : {}),
        label: typeof widget.label === 'string' ? widget.label : widget.name,
        optionLabel: (value) => {
            const current = isRecord(widget.options) ? widget.options : {};
            const label = typeof current.getOptionLabel === 'function' ? current.getOptionLabel(value) : value;
            return typeof label === 'string' ? label : value;
        },
        enhancementReady: () => trigger?.getAttribute('aria-expanded') !== 'true' &&
            !anchor.contains(anchor.ownerDocument.activeElement),
        readValue: () => (typeof props.modelValue === 'string' ? props.modelValue : ''),
        disabled: () => props.disabled === true ||
            props.readonly === true ||
            anchor.hasAttribute('readonly') ||
            trigger?.getAttribute('aria-disabled') === 'true',
        writeValue: (value) => {
            if (readValues().includes(value))
                emit.call(component, 'update:modelValue', value);
        },
        subscribe: (listener) => {
            const stop = watch.call(proxy, () => [props.modelValue, props.disabled, props.readonly, widget.options], listener, { flush: 'post' });
            if (typeof stop !== 'function')
                throw new TypeError('Comfy model field watcher returned an invalid disposer.');
            return () => stop();
        },
    };
}
/** Narrow mounted Vue roots at the dynamic host boundary. */
function elementOf(vnode) {
    const component = isRecord(vnode.component) ? vnode.component : {};
    const tree = isRecord(component.subTree) ? component.subTree : {};
    return tree.el instanceof HTMLElement ? tree.el : null;
}
