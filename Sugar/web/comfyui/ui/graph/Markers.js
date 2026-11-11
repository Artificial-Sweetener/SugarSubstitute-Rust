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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/Markers.js`.
 */
const WIDGET_NAME_ALIASES = Object.freeze({
    default_alias: ['default_alias', 'cube_name'],
});
function resolveWidgetNames(name) {
    return WIDGET_NAME_ALIASES[name] || [name];
}
function findWidget(node, name) {
    if (!node || !Array.isArray(node.widgets)) {
        return null;
    }
    const names = resolveWidgetNames(name);
    return node.widgets.find((entry) => entry && names.includes(entry.name)) || null;
}
/**
 * Read widget value.
 */
export function readWidgetValue(node, name) {
    const widget = findWidget(node, name);
    if (!widget) {
        return '';
    }
    return widget.value ?? widget.last_value ?? widget.options?.value;
}
/**
 * Write widget value.
 */
export function writeWidgetValue(node, name, value) {
    const widget = findWidget(node, name);
    if (!widget)
        return false;
    widget.value = value;
    if (typeof widget.callback === 'function') {
        try {
            widget.callback(value);
        }
        catch (_error) {
            // ignore widget callback failures
        }
    }
    return true;
}
