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
/** Identify native Vue presentation boundaries without touching custom widget internals. */
import { isRecord } from '../../types/common.js';
import { findVueComponents, findMountedNodeVNode } from '../ComfyVueTree.js';
import { classifyNativeField, planNativeFields, NATIVE_FIELD_COMPONENTS, } from './NativeFieldPresentation.js';
import { readNativeNumericField } from './ComfyNativeNumericField.js';
import { ComfyNativeSeedControlAdapter } from './ComfyNativeSeedControl.js';
import { readNativeWidgetMount } from '../NativeWidgetMountBoundary.js';
import { NativeFluentAnnotations, } from './NativeFluentAnnotations.js';
/** Own removable annotations on recognized Comfy component roots only. */
export class NativeFluentCardAdapter {
    #annotations = new NativeFluentAnnotations();
    #vnodes = new WeakMap();
    #graphNodes = new Map();
    #seeds;
    /** Resolve native execution controls through the supplied Comfy application boundary. */
    constructor(app = null) {
        this.#seeds = new ComfyNativeSeedControlAdapter(app);
    }
    /** Index graph component roots once for a coalesced host update. */
    beginBatch(documentRef, roots) {
        this.#seeds.beginBatch();
        const unresolved = roots.filter((root) => {
            if (this.#vnodes.has(root))
                return false;
            const vnode = findMountedNodeVNode(root);
            if (vnode)
                this.#vnodes.set(root, vnode);
            return !vnode;
        });
        if (!unresolved.length)
            return;
        const mount = documentRef.getElementById('vue-app');
        const record = isRecord(mount) ? mount : {};
        for (const vnode of findVueComponents(record._vnode, 'LGraphNode')) {
            const element = resolveVNodeElement(vnode);
            if (element && unresolved.includes(element))
                this.#graphNodes.set(element, vnode);
        }
    }
    /** Release the graph index so detached application trees are not retained. */
    endBatch() {
        this.#annotations.prune();
        this.#graphNodes.clear();
        this.#seeds.endBatch();
    }
    /** Annotate toolbar widgets through the same component and numeric boundaries as node fields. */
    annotateWidget(root) {
        const annotated = this.#annotations.begin(root);
        mark(root, 'node', annotated);
        const boundary = readNativeWidgetMount(root);
        const numericFields = [];
        const seedFields = [];
        const labelElements = [];
        const descriptor = resolveFieldDescriptor(root, indexFieldComponents(boundary?.vnode ?? {}), annotated, numericFields, labelElements, seedFields, this.#seeds);
        if (descriptor) {
            if (!root.hasAttribute('data-sc-choice-inline'))
                mark(root, 'row', annotated);
            annotated.attribute(root, 'data-sc-fluent-widget', descriptor.component);
        }
        if (boundary?.seed && numericFields[0])
            seedFields.push({ numeric: numericFields[0], control: boundary.seed });
        annotated.commit();
        return {
            root,
            body: null,
            fields: descriptor ? [descriptor] : [],
            numericFields,
            seedFields,
            labelElements,
            header: null,
            collapsed: false,
            canCollapse: false,
            advanced: null,
        };
    }
    /** Mark one live card and its positively identified native field rows. */
    annotate(root) {
        const annotated = this.#annotations.begin(root);
        mark(root, 'node', annotated);
        const vnode = this.#vnodes.get(root) ?? this.#graphNodes.get(root) ?? resolveNodeVNode(root);
        if (vnode)
            this.#vnodes.set(root, vnode);
        const descriptors = [];
        const numericFields = [];
        const seedFields = [];
        const labelElements = [];
        for (const widgets of findVueComponents(vnode, 'NodeWidgets')) {
            const container = resolveVNodeElement(widgets);
            if (!container || container.closest('.lg-node') !== root)
                continue;
            mark(container, 'fields', annotated);
            const containerFields = [];
            const candidates = indexFieldComponents(widgets);
            const rows = new Map();
            for (const child of Array.from(container.children)) {
                if (!(child instanceof HTMLElement))
                    continue;
                const descriptor = resolveFieldDescriptor(child, candidates, annotated, numericFields, labelElements, seedFields, this.#seeds);
                const advanced = candidates.some(({ vnode: candidate }) => {
                    const component = isRecord(candidate.component) ? candidate.component : {};
                    const props = isRecord(component.props) ? component.props : {};
                    const widget = isRecord(props.widget) ? props.widget : {};
                    const options = isRecord(widget.options) ? widget.options : {};
                    const element = resolveVNodeElement(candidate);
                    return options.advanced === true && element !== null && child.contains(element);
                });
                if (advanced)
                    mark(child, 'advanced-row', annotated);
                if (!descriptor) {
                    containerFields.push({ name: '', component: '' });
                    continue;
                }
                if (!child.hasAttribute('data-sc-search-field-hidden'))
                    containerFields.push(descriptor);
                rows.set(descriptor.name, child);
            }
            descriptors.push(...containerFields);
            for (const placement of planNativeFields(containerFields)) {
                const row = rows.get(placement.name);
                if (!row)
                    continue;
                mark(row, 'row', annotated);
                annotated.attribute(row, 'data-sc-fluent-kind', placement.kind);
                annotated.attribute(row, 'data-sc-fluent-span', String(placement.span));
                annotated.attribute(row, 'data-sc-fluent-column', String(placement.column));
                annotated.attribute(row, 'data-sc-fluent-widget', containerFields.find((field) => field.name === placement.name)?.component ?? '');
                annotated.style(row, '--sc-field-row', String(placement.row));
                annotated.style(row, '--sc-field-column', String(placement.column));
                annotated.style(row, '--sc-field-span', String(placement.span));
            }
        }
        const headerVNode = findVueComponents(vnode, 'NodeHeader')[0];
        const header = headerVNode ? resolveVNodeElement(headerVNode) : null;
        const title = header?.querySelector('[data-testid="node-title"] .editable-text > span');
        if (title)
            labelElements.push(title);
        const headerComponent = isRecord(headerVNode?.component) ? headerVNode.component : {};
        const headerProps = isRecord(headerComponent.props) ? headerComponent.props : {};
        const nodeComponent = isRecord(vnode?.component) ? vnode.component : {};
        const nodeProps = isRecord(nodeComponent.props) ? nodeComponent.props : {};
        const nodeData = isRecord(nodeProps.nodeData) ? nodeProps.nodeData : {};
        const footer = findVueComponents(vnode, 'NodeFooter')[0];
        const footerComponent = isRecord(footer?.component) ? footer.component : {};
        const footerProps = isRecord(footerComponent.props) ? footerComponent.props : {};
        const button = [...root.querySelectorAll('[data-testid="advanced-inputs-button"]')].find((element) => element.closest('.lg-node') === root) ?? null;
        const fields = [...root.querySelectorAll('[data-sc-fluent-fields]')].find((element) => element.closest('.lg-node') === root) ?? null;
        const visibleFields = Array.from(fields?.children ?? []).some((child) => child instanceof HTMLElement && !child.hidden && child.style.display !== 'none');
        if (visibleFields && !root.closest('.sugarcubes-cube-face__node-card')) {
            for (const slots of findVueComponents(vnode, 'NodeSlots')) {
                const container = resolveVNodeElement(slots);
                if (container?.closest('.lg-node') === root && container.querySelector('.lg-slot'))
                    mark(container, 'ports', annotated);
            }
        }
        annotated.commit();
        return {
            root,
            header,
            collapsed: headerProps.collapsed === true,
            canCollapse: root.dataset.cubeFacePresentation === 'true'
                ? root.querySelector('[data-testid="node-collapse-button"]')
                    ?.disabled !== true
                : hasNativeWidgets(nodeData.widgets) || footerProps.showAdvancedInputsButton === true,
            advanced: button && fields
                ? { button, fields, expanded: footerProps.showAdvancedState === true }
                : null,
            body: root.querySelector('[data-testid^="node-body-"]'),
            fields: descriptors,
            numericFields,
            seedFields,
            labelElements,
        };
    }
    /** Remove only owned annotations, preserving host and custom inline style changes. */
    clear() {
        this.#annotations.clear();
        this.endBatch();
    }
}
/** Read both native graph widget dictionaries and projected widget arrays. */
function hasNativeWidgets(widgets) {
    return Array.isArray(widgets)
        ? widgets.length > 0
        : isRecord(widgets) && Object.keys(widgets).length > 0;
}
/** Track annotations so turning the theme off restores host ownership exactly. */
function mark(element, name, annotated) {
    annotated.attribute(element, `data-sc-fluent-${name}`);
}
/** Resolve the actual node instance without crossing into a nested Cube card. */
function resolveNodeVNode(root) {
    const mount = root.closest('.sugarcubes-native-node-card') ?? root.ownerDocument.getElementById('vue-app');
    const record = isRecord(mount) ? mount : {};
    return (findVueComponents(record._vnode, 'LGraphNode').find((vnode) => resolveVNodeElement(vnode) === root) ?? null);
}
/** Read a core widget's identity and annotate its native layout, excluding DOM/custom bridges. */
function resolveFieldDescriptor(row, candidates, annotated, numericFields, labelElements, seedFields, seeds) {
    const contains = (vnode) => {
        const element = resolveVNodeElement(vnode);
        return element !== null && row.contains(element);
    };
    if (candidates.some(({ name, vnode }) => ['WidgetDOM', 'WidgetLegacy'].includes(name) && contains(vnode)))
        return null;
    for (const { name, vnode } of candidates) {
        if (!NATIVE_FIELD_COMPONENTS.includes(name) || !contains(vnode))
            continue;
        const component = isRecord(vnode.component) ? vnode.component : {};
        const props = isRecord(component.props) ? component.props : {};
        const widget = isRecord(props.widget) ? props.widget : {};
        if (typeof widget.name !== 'string')
            continue;
        const descriptor = { name: widget.name, component: name };
        if (!classifyNativeField(descriptor))
            continue;
        const control = resolveVNodeElement(vnode);
        const layoutVNode = findVueComponents(vnode, 'WidgetLayoutField')[0];
        const layout = layoutVNode ? resolveVNodeElement(layoutVNode) : null;
        if (control) {
            mark(control, 'control', annotated);
            const input = control.querySelector('input:not([type="checkbox"]), textarea');
            const frame = name === 'WidgetTextarea'
                ? control
                : name.startsWith('WidgetInputNumber')
                    ? input?.closest('[aria-label]')
                    : name === 'WidgetInputText'
                        ? input?.parentElement
                        : name === 'WidgetSelect'
                            ? control
                                .querySelector('[data-testid="widget-select-default-trigger"]')
                                ?.closest('[data-capture-wheel]')
                            : null;
            if (frame && control.contains(frame))
                mark(frame, 'edit-frame', annotated);
            if (name === 'WidgetSelect' && frame && control.contains(frame))
                mark(frame, 'combo-frame', annotated);
        }
        if (layout) {
            mark(layout, 'layout', annotated);
            const label = layout.querySelector('[data-testid="widget-layout-field-label"]');
            if (label)
                labelElements.push(label);
        }
        if (name.startsWith('WidgetInputNumber')) {
            const numeric = readNativeNumericField(row, vnode);
            if (numeric) {
                numericFields.push(numeric);
                const control = seeds.read(widget, numeric.precision);
                if (control)
                    seedFields.push({ numeric, control });
            }
        }
        return descriptor;
    }
    return null;
}
/** Discover native and custom bridge capabilities once per widget container. */
function indexFieldComponents(widgets) {
    return [...NATIVE_FIELD_COMPONENTS, 'WidgetDOM', 'WidgetLegacy'].flatMap((name) => findVueComponents(widgets, name).map((vnode) => ({ name, vnode })));
}
/** Narrow the root element of a mounted native component. */
function resolveVNodeElement(vnode) {
    const component = isRecord(vnode.component) ? vnode.component : {};
    const subTree = isRecord(component.subTree) ? component.subTree : {};
    return subTree.el instanceof HTMLElement ? subTree.el : null;
}
