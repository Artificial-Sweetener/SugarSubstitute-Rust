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
/**
 * Own low-level writes from prepared import entries into live ComfyUI nodes.
 */
import { writeWidgetValue } from '../graph/Markers.js';
import { isRecord } from '../types/common.js';
/** Place fallback nodes on a stable three-column grid. */
export function computeGridPosition(origin, index) {
    const baseX = Number(origin[0]) || 0;
    const baseY = Number(origin[1]) || 0;
    const columns = 3;
    const column = index % columns;
    const row = Math.floor(index / columns);
    return [baseX + column * 320, baseY + row * 240];
}
/** Expand aggregate import bounds to include one live node. */
export function updateBoundsWithNode(bounds, node) {
    const pos = Array.isArray(node.pos) ? node.pos : [0, 0];
    const size = Array.isArray(node.size) ? node.size : [140, 60];
    const x = Number(pos[0]) || 0;
    const y = Number(pos[1]) || 0;
    const width = Number(size[0]) || 140;
    const height = Number(size[1]) || 60;
    bounds.minX = Math.min(bounds.minX, x);
    bounds.minY = Math.min(bounds.minY, y);
    bounds.maxX = Math.max(bounds.maxX, x + width);
    bounds.maxY = Math.max(bounds.maxY, y + height);
}
/** Resolve an output name or numeric specification to a slot index. */
export function resolveOutputSlotIndex(node, slotSpec) {
    if (typeof slotSpec === 'number' && Number.isFinite(slotSpec))
        return slotSpec;
    if (typeof slotSpec === 'string' && slotSpec) {
        const outputs = node.outputs ?? [];
        const byName = outputs.findIndex((output) => output.name === slotSpec);
        if (byName !== -1)
            return byName;
        const parsed = Number(slotSpec);
        if (Number.isFinite(parsed))
            return parsed;
    }
    return 0;
}
/** Find a named input slot on one live node. */
export function resolveInputSlotIndex(node, inputName) {
    if (!Array.isArray(node.inputs))
        return -1;
    return node.inputs.findIndex((input) => input.name === inputName);
}
/** Ensure a prepared connection can target a named node input. */
export function ensureInputSlot(node, inputName) {
    if (resolveInputSlotIndex(node, inputName) !== -1)
        return true;
    const widget = Array.isArray(node.widgets)
        ? node.widgets.find((entry) => entry && entry.name === inputName)
        : null;
    if (widget && typeof node.convertWidgetToInput === 'function') {
        try {
            if (node.convertWidgetToInput(widget) !== false)
                return true;
        }
        catch (_error) {
            // Host widget conversions are optional and may reject unsupported inputs.
        }
        return resolveInputSlotIndex(node, inputName) !== -1;
    }
    return false;
}
/** Apply one prepared input value through widget or property ownership. */
export function applyInputValueToNode(node, inputName, value) {
    if (writeWidgetValue(node, inputName, value))
        return true;
    if (typeof node.setProperty === 'function') {
        try {
            node.setProperty(inputName, value);
            return true;
        }
        catch (_error) {
            // Fall through to direct ownership for hosts that reject setProperty.
        }
    }
    if (!isRecord(node.properties))
        node.properties = {};
    if (!Object.prototype.hasOwnProperty.call(node.properties, inputName))
        return false;
    node.properties[inputName] = value;
    if (typeof node.onPropertyChanged === 'function') {
        try {
            node.onPropertyChanged(inputName, value);
        }
        catch (_error) {
            // The value is already authoritative even when the host callback fails.
        }
    }
    return true;
}
/** Apply serialized widget and property extras to one imported node. */
export function applyExtrasToNode(node, extras) {
    if (Array.isArray(extras.widgets_values) && Array.isArray(node.widgets)) {
        for (let index = 0; index < node.widgets.length && index < extras.widgets_values.length; index += 1) {
            const widget = node.widgets[index];
            if (!widget)
                continue;
            const value = extras.widgets_values[index];
            widget.value = value;
            if (typeof widget.callback === 'function') {
                try {
                    widget.callback(value);
                }
                catch (_error) {
                    // Continue applying the remaining serialized widget values.
                }
            }
        }
    }
    if (!isRecord(extras.properties))
        return;
    for (const [key, propertyValue] of Object.entries(extras.properties)) {
        if (typeof node.setProperty === 'function') {
            try {
                node.setProperty(key, propertyValue);
                continue;
            }
            catch (_error) {
                // Fall through to the node's property record.
            }
        }
        if (!isRecord(node.properties))
            node.properties = {};
        node.properties[key] = propertyValue;
        if (typeof node.onPropertyChanged === 'function') {
            try {
                node.onPropertyChanged(key, propertyValue);
            }
            catch (_error) {
                // Continue applying the remaining serialized properties.
            }
        }
    }
}
/** Apply a positive serialized execution mode to one imported node. */
export function applyExecutionMode(node, value) {
    if (typeof value === 'number' && Number.isInteger(value) && value > 0)
        node.mode = value;
}
