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
/** Pack compact squares together while retaining expanded row spacing. */
export function cubeStackRowGap(compact: boolean): number {
  return compact ? 0 : CUBE_STACK_ROW_GAP;
}

/** Advance one settled slot independently of an in-flight visual transform. */
export function cubeStackRowStride(compact: boolean): number {
  return CUBE_STACK_ROW_HEIGHT + cubeStackRowGap(compact);
}

/** Reserve only the tile width for compact navigation and retain expanded labels. */
export function cubeStackWidth(compact: boolean): number {
  return compact ? CUBE_STACK_ROW_HEIGHT : 224;
}

/** Separate the expanded rail from the masonry cards it reserves space beside. */
export const CUBE_STACK_MASONRY_GUTTER = 24;
