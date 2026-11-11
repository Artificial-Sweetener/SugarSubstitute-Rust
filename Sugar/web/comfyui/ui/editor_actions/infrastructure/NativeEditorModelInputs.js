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
/** Read generative-model candidates independently of the visibility of native combo controls. */
import { isRecord } from '../../types/common.js';
import { readNativeWidgetOrigins } from '../../surface/NativeWidgetOrigins.js';
/** Prefer the native reactive getter while retaining declared values for collapsed or advanced fields. */
export function readNativeEditorModelInputs(widgets, fields) {
    if (!Array.isArray(widgets))
        return [];
    const result = [];
    for (const widget of widgets) {
        if (!isRecord(widget) || typeof widget.name !== 'string' || widget.serialize === false)
            continue;
        const options = () => {
            const values = isRecord(widget.options) ? widget.options.values : undefined;
            return Array.isArray(values) && values.every((value) => typeof value === 'string')
                ? values.filter((value) => typeof value === 'string')
                : [];
        };
        if (widget.type !== 'combo' && !options().length)
            continue;
        const field = fields.find((field) => field.name === widget.name);
        result.push({
            name: widget.name,
            origins: readNativeWidgetOrigins(widget),
            options,
            read: field ? field.read : () => widget.value,
        });
    }
    return result;
}
