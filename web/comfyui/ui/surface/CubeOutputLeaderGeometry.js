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
/** Define the renderer-neutral geometry invariant for output association leaders. */
/** Keep an output socket level with or below the center of its associated label. */
export function resolveCubeOutputPortY(labelY, requestedPortY) {
    return Math.max(labelY, requestedPortY);
}
/** Resolve a straight or downward-stepping orthogonal output association leader. */
export function resolveCubeOutputLeader(labelEndX, labelY, portX, requestedPortY, elbowX) {
    const portY = resolveCubeOutputPortY(labelY, requestedPortY);
    return [
        [labelEndX, labelY],
        [elbowX, labelY],
        [elbowX, portY],
        [portX, portY],
    ];
}
