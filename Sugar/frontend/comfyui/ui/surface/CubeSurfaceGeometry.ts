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
/** Own shared static spacing used by Cube face geometry policies. */

/** Inset ordinary face content from an edge without a boundary gutter. */
export const CUBE_SURFACE_FRAME_PADDING = 12;

/** Separate the masonry controls from the output preview. */
export const CUBE_SURFACE_SECTION_GAP = 12;

/** Separate Nodes 2.0 masonry from its preview rail. */
export const CUBE_DOM_SURFACE_SECTION_GAP = 8;

/** Account for Comfy's Nodes 2.0 horizontal body inset around custom content. */
export const CUBE_VUE_CONTENT_HORIZONTAL_INSET = 16;
