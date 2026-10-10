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
/** Advance one settled slot regardless of an in-flight visual transform. */
export const CUBE_STACK_ROW_STRIDE = CUBE_STACK_ROW_HEIGHT + CUBE_STACK_ROW_GAP;
