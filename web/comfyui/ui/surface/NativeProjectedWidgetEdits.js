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
/** Commit copied native Vue controls to their owning widgets inside native history. */
import { isRecord } from '../types/common.js';
/** Preserve extracted host metadata while replacing copy-only edits with authoritative writes. */
export function projectNativeWidgetEdits(node, widgets) {
    if (!Array.isArray(widgets))
        return widgets;
    return widgets.map((projected) => {
        if (!isRecord(projected) || typeof projected.name !== 'string')
            return projected;
        const owner = node.widgets?.find((widget) => widget.name === projected.name);
        if (!owner)
            return projected;
        const callback = projected.callback;
        return {
            ...projected,
            /** Capture the complete native value change before exposing its completed history state. */
            callback(value) {
                const root = isRecord(node.rootGraph) ? node.rootGraph : node.graph;
                const before = root?.beforeChange;
                const after = root?.afterChange;
                const transactional = typeof before === 'function' && typeof after === 'function';
                if (transactional)
                    before.call(root);
                try {
                    owner.value = value;
                    if (typeof callback === 'function')
                        callback.call(projected, value);
                }
                finally {
                    if (transactional)
                        after.call(root);
                }
            },
        };
    });
}
