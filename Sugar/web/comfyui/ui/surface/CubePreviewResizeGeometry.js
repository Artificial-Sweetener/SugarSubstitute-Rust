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
/** Define the shared width constraints for Cube output-rail resizing. */
/** Keep output actions and media usable at the narrowest authored rail. */
export const CUBE_PREVIEW_MINIMUM_WIDTH = 160;
/** Resolve a finite range while preserving the minimum masonry column. */
export function resolveCubePreviewWidthRange(availableWidth, minimumMasonryWidth, gap) {
    const maximum = Math.max(0, availableWidth - Math.max(0, gap) - minimumMasonryWidth);
    return {
        minimum: Math.min(CUBE_PREVIEW_MINIMUM_WIDTH, maximum),
        maximum,
    };
}
/** Clamp a drag-derived width to the renderer-neutral output-rail range. */
export function clampCubePreviewWidth(width, range) {
    const finite = Number.isFinite(width) ? width : range.minimum;
    return Math.min(range.maximum, Math.max(range.minimum, finite));
}
