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
/** Project validated catalog images into the shared gallery and keyboard geometry. */
import { chooseModelThumbnail } from './ModelCatalog.js';
import { layoutModelWall, MODEL_PICKER_WALL_PROFILE } from './ModelPickerLayout.js';
const shapes = new WeakMap();
/** A tile's width over its height: the largest standard image's, so decode size cannot change it. */
function tileShape(item) {
    let shape = shapes.get(item);
    if (shape === undefined) {
        const image = chooseModelThumbnail(item, 'standard', Number.POSITIVE_INFINITY);
        shape = image ? image.width / image.height : 0.72;
        shapes.set(item, shape);
    }
    return shape;
}
let laid = null;
/**
 * Lay a wall out from its tiles' shapes. The catalog republishes its models whenever a name or a
 * picture arrives, almost always leaving every shape as it was; the last layout is then returned
 * as it is, so a wall of thousands of tiles is not measured again and its rows keep their identity.
 */
export function layoutModelGallery(items, width, profile = MODEL_PICKER_WALL_PROFILE) {
    const wanted = items.map(tileShape);
    if (laid?.width === width &&
        laid.profile === profile &&
        laid.shapes.length === wanted.length &&
        laid.shapes.every((shape, index) => shape === wanted[index]))
        return laid.rows;
    laid = { shapes: wanted, width, profile, rows: layoutModelWall(wanted, width, profile) };
    return laid.rows;
}
