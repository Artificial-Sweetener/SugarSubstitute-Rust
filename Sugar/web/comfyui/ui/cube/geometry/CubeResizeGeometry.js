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
/** Compute anchored native Cube-node resizing independently of a renderer or pointer source. */
/** Enumerate every supported Cube edge and corner resize direction. */
export const CUBE_RESIZE_EDGES = ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw'];
/** Resize one frame while keeping every unaffected opposite edge fixed. */
export function resizeCubeFrame(options) {
    const [startX, startY] = options.startPosition;
    const [startWidth, startHeight] = options.startSize;
    const [deltaX, deltaY] = options.delta;
    const [minimumWidth, minimumHeight] = options.minimumSize;
    const movesLeft = options.edge.includes('w');
    const movesRight = options.edge.includes('e');
    const movesTop = options.edge.includes('n');
    const movesBottom = options.edge.includes('s');
    const width = Math.max(minimumWidth, movesLeft ? startWidth - deltaX : movesRight ? startWidth + deltaX : startWidth);
    const height = Math.max(minimumHeight, movesTop ? startHeight - deltaY : movesBottom ? startHeight + deltaY : startHeight);
    return {
        position: [
            movesLeft ? startX + startWidth - width : startX,
            movesTop ? startY + startHeight - height : startY,
        ],
        size: [width, height],
    };
}
/** Validate one dynamic resize-edge marker at the host boundary. */
export function isCubeResizeEdge(value) {
    return typeof value === 'string' && CUBE_RESIZE_EDGES.includes(value);
}
