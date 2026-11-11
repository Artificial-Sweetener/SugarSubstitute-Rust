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
/** Own stacking levels shared by SugarCubes workspace surfaces, dialogs and detached controls. */

/** Place modal backdrops above the surrounding Comfy interface. */
export const DIALOG_OVERLAY_Z_INDEX = 2_000;

/** Keep detached Settings control overlays interactive above modal backdrops. */
export const COMFY_SETTINGS_OVERLAY_BASE_Z_INDEX = 2_100;

/** Keep masonry presentation above the native graph with its rail immediately above the backdrop. */
export const MASONRY_SURFACE_Z_INDEX = 1_100;
/** Keep the rail interactive while finder and dialog overlays remain above it. */
export const MASONRY_STACK_Z_INDEX = MASONRY_SURFACE_Z_INDEX + 1;
