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
/** Own small record and width operations shared by Cube surface presentation. */
import { CUBE_INPUT_GUTTER_WIDTH } from './CubePortGutterLayout.js';
import { CUBE_VUE_CONTENT_HORIZONTAL_INSET } from './CubeSurfaceGeometry.js';
/** Replace persisted surface state while retaining domain ownership of the record. */
export function replaceCubeSurfaceRecord(target, source) {
    for (const key of Object.keys(target))
        Reflect.deleteProperty(target, key);
    Object.assign(target, source);
}
/** Exclude only input labels because output slots overlay the preview's right rail. */
export function resolveCubeVueContentWidth(node) {
    const inputGutterWidth = node.inputs.length > 0 ? CUBE_INPUT_GUTTER_WIDTH : 0;
    return Math.max(1, Number(node.size[0]) - CUBE_VUE_CONTENT_HORIZONTAL_INSET - inputGutterWidth);
}
