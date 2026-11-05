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
/** Share settled rail slot geometry between Vue positioning, pointer hit testing, and styling. */
export const CUBE_STACK_ROW_HEIGHT = 46;
/** Retain the reference rail's spacing between adjacent rows. */
export const CUBE_STACK_ROW_GAP = 4;
/** Inset rows from the rail edges unless masonry packing places them flush. */
export const CUBE_STACK_ROW_INSET = 6;
/** Retain the expanded rail width; compact rows only drop their labels. */
export const CUBE_STACK_EXPANDED_WIDTH = 224;
/** Separate a floating rail from the masonry edge and the first card column. */
const FLOATING_RAIL_MARGIN = 12;
/** Touch adjacent rows only while masonry packing is active. */
export function cubeStackRowGap(packed) {
    return packed ? 0 : CUBE_STACK_ROW_GAP;
}
/** Advance one settled slot independently of an in-flight visual transform. */
export function cubeStackRowStride(packed) {
    return CUBE_STACK_ROW_HEIGHT + cubeStackRowGap(packed);
}
/** Size the rail: compact keeps one icon tile plus the row insets that packing removes. */
export function cubeStackWidth(compact, packed) {
    if (!compact)
        return CUBE_STACK_EXPANDED_WIDTH;
    return CUBE_STACK_ROW_HEIGHT + (packed ? 0 : CUBE_STACK_ROW_INSET * 2);
}
/** Reserve masonry width for the rail and, when it floats, its outer and inner margins. */
export function cubeStackReservedWidth(compact, packed, framed) {
    return cubeStackWidth(compact, packed) + (packed || framed ? 0 : FLOATING_RAIL_MARGIN * 2);
}
