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
/** Own Nodes 1.0 socket centers and visible rim geometry. */
/** Preserve LiteGraph's native slot hit-target height. */
export const CUBE_CANVAS_SLOT_HEIGHT = 20;
/** Preserve LiteGraph's visible native socket radius. */
export const CUBE_CANVAS_SOCKET_RADIUS = 4;
/** Match LiteGraph's native output-slot center inset. */
export function resolveCubeCanvasOutputSocketCenterX(frameRight, slotHeight = CUBE_CANVAS_SLOT_HEIGHT) {
    return frameRight + 1 - slotHeight / 2;
}
/** Match LiteGraph's native input-slot center inset. */
export function resolveCubeCanvasInputSocketCenterX(frameLeft, slotHeight = CUBE_CANVAS_SLOT_HEIGHT) {
    return frameLeft + slotHeight / 2 - 1;
}
/** Stop an association leader at the visible left rim of an output socket. */
export function resolveCubeCanvasOutputSocketRimX(frameRight, slotHeight = CUBE_CANVAS_SLOT_HEIGHT, socketRadius = CUBE_CANVAS_SOCKET_RADIUS) {
    return resolveCubeCanvasOutputSocketCenterX(frameRight, slotHeight) - socketRadius;
}
