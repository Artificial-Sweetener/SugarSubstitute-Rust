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
/** Reserve one normal LiteGraph slot row above slot-suppressed Cube-face widgets. */
export const CUBE_FACE_WIDGET_GUTTER = 20;
const NATIVE_WIDGET_INSET = 2;
/** Reserve a hidden-slot row only for widgets that do not provide their own native top placement. */
export function cubeFaceNodeWidgetGutter(node) {
    return findCubeFacePromptWidget(node) ? 0 : CUBE_FACE_WIDGET_GUTTER;
}
/** Create face flags that retain native state except graph collapse. */
function createCubeFaceFlags(flags) {
    return {
        ...(isRecord(flags) ? flags : {}),
        collapsed: false,
    };
}
/** Create safe Comfy Vue render data without any internal graph boundary slots. */
export function createCubeFaceNodeData(nodeData) {
    return {
        ...nodeData,
        flags: createCubeFaceFlags(nodeData.flags),
        inputs: [],
        outputs: [],
    };
}
/** Expose expanded slotless state during one native operation, then restore exact ownership. */
export function withCubeFaceNodePresentation(node, operation) {
    const remembered = rememberNodeProperties(node, ['flags', 'widgets_start_y', 'widgets_up']);
    node.flags = createCubeFaceFlags(node.flags);
    Reflect.set(node, 'widgets_up', true);
    // Preserve the first native slot row only when its widget does not already own that top spacing.
    Reflect.set(node, 'widgets_start_y', cubeFaceNodeWidgetGutter(node) + NATIVE_WIDGET_INSET);
    try {
        return operation();
    }
    finally {
        restoreNodeProperties(node, remembered);
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
    const widgets = node.widgets ?? [];
    if (widgets.length === 0)
        return false;
    const isWidgetVisible = node.isWidgetVisible;
    if (typeof isWidgetVisible !== 'function')
        return true;
    return widgets.some((widget) => isWidgetVisible.call(node, widget) !== false);
}
