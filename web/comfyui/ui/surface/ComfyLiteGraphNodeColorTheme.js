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
/** Adapt the active LiteGraph scene palette into Cube surface colors. */
const LITEGRAPH_NATIVE_DEFAULT_THEME = {
    header: '#333',
    body: '#353535',
};
/** Resolve one Cube shell from explicit node colors and the live LiteGraph palette. */
export function resolveComfyLiteGraphCubeSurfaceTheme(node, source = globalThis.LiteGraph) {
    return {
        header: readColor(Reflect.get(node, 'color')) ??
            readColor(source?.NODE_DEFAULT_COLOR) ??
            LITEGRAPH_NATIVE_DEFAULT_THEME.header,
        body: readColor(Reflect.get(node, 'bgcolor')) ??
            readColor(source?.NODE_DEFAULT_BGCOLOR) ??
            LITEGRAPH_NATIVE_DEFAULT_THEME.body,
    };
}
/** Narrow one dynamic host color to a non-empty Canvas color string. */
function readColor(value) {
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}
