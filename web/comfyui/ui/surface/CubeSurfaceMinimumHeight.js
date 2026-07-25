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
/** Own the renderer-independent minimum-height rule for Cube faces. */
/** Preserve Comfy's established minimum width for a usable Cube face. */
export const CUBE_MINIMUM_WIDTH = 320;
/** Preserve the smallest useful Cube height when its face content is shorter. */
export const CUBE_BASE_MINIMUM_HEIGHT = 180;
/** Fit all face content between equal vertical gutters without vertical scrolling. */
export function resolveCubeSurfaceMinimumHeight(options) {
    const contentHeight = finiteNonNegative(options.contentHeight);
    const headerInset = finiteNonNegative(options.headerInset);
    const footerInset = finiteNonNegative(options.footerInset);
    const faceHeaderHeight = finiteNonNegative(options.faceHeaderHeight ?? 0);
    const nativeTitleHeight = finiteNonNegative(options.nativeTitleHeight ?? 0);
    return Math.ceil(Math.max(CUBE_BASE_MINIMUM_HEIGHT, contentHeight + headerInset + footerInset + Math.max(0, faceHeaderHeight - nativeTitleHeight)));
}
/** Return the shared minimum Cube size for one responsive face height. */
export function cubeMinimumSize(minimumHeight) {
    return [CUBE_MINIMUM_WIDTH, Math.max(CUBE_BASE_MINIMUM_HEIGHT, finiteNonNegative(minimumHeight))];
}
/** Keep dynamic layout values finite before they reach graph geometry. */
function finiteNonNegative(value) {
    return Number.isFinite(value) ? Math.max(0, value) : 0;
}
