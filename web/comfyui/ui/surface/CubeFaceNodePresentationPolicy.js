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
/** Own transient native-node state used only while presenting Cube-face cards. */
import { isRecord } from '../types/common.js';
import { findCubeFacePromptWidget } from './CubeFacePromptPolicy.js';
import { cubeFaceUnconsumedWidgets, cubeFaceVisibleWidgets } from './CubeFaceWidgetPolicy.js';
/** Match the compact native bottom margin above ordinary Cube-face widget stacks. */
export const CUBE_FACE_WIDGET_PADDING = 6;
const PROMPT_WIDGET_TOP_PADDING = 2;
/** Resolve balanced compact padding while preserving multiline prompt placement. */
export function cubeFaceNodeWidgetStartY(node) {
    return findCubeFacePromptWidget(node) ? PROMPT_WIDGET_TOP_PADDING : CUBE_FACE_WIDGET_PADDING;
}
/** Create face flags that retain native state except graph collapse. */
function createCubeFaceFlags(flags) {
    return {
        ...(isRecord(flags) ? flags : {}),
        collapsed: false,
    };
}
/** Create safe Comfy Vue render data with only controls still owned by the node. */
export function createCubeFaceNodeData(nodeData, node) {
    return {
        ...nodeData,
        flags: createCubeFaceFlags(nodeData.flags),
        inputs: [],
        outputs: [],
        widgets: cubeFaceVisibleWidgets(node),
    };
}
/** Expose expanded slotless state during one native operation, then restore exact ownership. */
export function withCubeFaceNodePresentation(node, operation) {
    const remembered = rememberNodeProperties(node, [
        'flags',
        'widgets',
        'widgets_start_y',
        'widgets_up',
    ]);
    node.widgets = cubeFaceUnconsumedWidgets(node);
    const widgetLabels = applyDurableWidgetLabels(node);
    node.flags = createCubeFaceFlags(node.flags);
    Reflect.set(node, 'widgets_up', true);
    Reflect.set(node, 'widgets_start_y', cubeFaceNodeWidgetStartY(node));
    try {
        return operation();
    }
    finally {
        restoreWidgetLabels(widgetLabels);
        restoreNodeProperties(node, remembered);
    }
}
/** Prefer a durable user-facing tooltip when a transient canvas label was lost. */
function applyDurableWidgetLabels(node) {
    const remembered = [];
    for (const widget of node.widgets ?? []) {
        const tooltip = widget.options?.tooltip;
        if (typeof tooltip !== 'string' || tooltip.trim() === '')
            continue;
        if (typeof widget.label === 'string' && widget.label !== widget.name)
            continue;
        remembered.push({
            widget,
            owned: Object.prototype.hasOwnProperty.call(widget, 'label'),
            value: widget.label,
        });
        widget.label = tooltip;
    }
    return remembered;
}
/** Restore widget labels without manufacturing persistent graph state. */
function restoreWidgetLabels(remembered) {
    for (const entry of remembered) {
        Reflect.set(entry.widget, 'label', entry.value);
        if (!entry.owned)
            Reflect.deleteProperty(entry.widget, 'label');
    }
}
/** Capture exact own-property semantics for every temporary face override. */
function rememberNodeProperties(node, keys) {
    return keys.map((key) => ({
        key,
        owned: Object.prototype.hasOwnProperty.call(node, key),
        value: node[key],
    }));
}
/** Restore temporary face properties without manufacturing inherited state. */
function restoreNodeProperties(node, remembered) {
    for (const property of remembered) {
        Reflect.set(node, property.key, property.value);
        if (!property.owned)
            Reflect.deleteProperty(node, property.key);
    }
}
/** Return whether Comfy will present at least one real widget control. */
export function cubeFaceNodeHasVisibleWidgets(node) {
    return cubeFaceVisibleWidgets(node).length > 0;
}
