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
/** Measure native Nodes 2.0 Cube boundary geometry before presentation mutates layout. */
const SOCKET_EDGE_INSET = 12;
const DEFAULT_SOCKET_RADIUS = 6;
/** Convert DOM-root offsets into node-local graph coordinates and CSS row coordinates. */
export function measureCubeBoundary(body, nodeRoot, row, slots, measuredRowPositions, nodeTitleHeight) {
    const graphOriginOffset = finite(nodeTitleHeight, 30);
    const rowOffset = readLocalOffset(nodeRoot, row);
    const bodyOffset = readLocalOffset(nodeRoot, body);
    const bodyHeight = readLocalHeight(body, body);
    const rootMinimumY = bodyOffset + SOCKET_EDGE_INSET;
    const rootMaximumY = Math.max(rootMinimumY, bodyOffset + bodyHeight - SOCKET_EDGE_INSET);
    const minimumY = rootMinimumY - graphOriginOffset;
    const maximumY = rootMaximumY - graphOriginOffset;
    const nativePositions = slots.map((slot) => readCenterOffset(row, slot));
    const anchors = slots.map((_, index) => {
        const rowY = measuredRowPositions[index] ?? nativePositions[index] ?? SOCKET_EDGE_INSET;
        const defaultY = clamp(rowOffset + rowY - graphOriginOffset, minimumY, maximumY);
        return { index, defaultY, minY: minimumY, maxY: maximumY, labelY: defaultY };
    });
    return {
        anchors,
        graphOriginOffset,
        rowOffset,
        rowMinimumY: rootMinimumY - rowOffset,
        rowMaximumY: rootMaximumY - rowOffset,
    };
}
/** Measure one rendered connection-dot radius in unscaled CSS coordinates. */
export function measureSocketRadius(body, slot) {
    const dot = slot?.querySelector('[data-testid="slot-connection-dot"]');
    if (!dot)
        return DEFAULT_SOCKET_RADIUS;
    const scale = readScale(body);
    const radius = dot.getBoundingClientRect().width / scale / 2;
    return Number.isFinite(radius) && radius > 0 ? radius : DEFAULT_SOCKET_RADIUS;
}
/** Convert one descendant's top offset into its ancestor's unscaled coordinates. */
export function readLocalOffset(ancestor, descendant) {
    const ancestorRect = ancestor.getBoundingClientRect();
    const descendantRect = descendant.getBoundingClientRect();
    return (descendantRect.top - ancestorRect.top) / readScale(ancestor);
}
/** Read the graph-space Y represented by Comfy's title-offset DOM transform. */
export function readVueNodeGraphY(nodeRoot, nodeTitleHeight, fallback) {
    const transform = nodeRoot.style.transform;
    const translate = /translate(?:3d)?\(\s*[^,]+,\s*(-?\d+(?:\.\d+)?)px(?:,|\))/u.exec(transform);
    if (translate?.[1] !== undefined) {
        return finite(translate[1], fallback - nodeTitleHeight) + nodeTitleHeight;
    }
    const matrix = /matrix\(\s*[^,]+,\s*[^,]+,\s*[^,]+,\s*[^,]+,\s*[^,]+,\s*(-?\d+(?:\.\d+)?)\s*\)/u.exec(transform);
    return matrix?.[1] === undefined
        ? fallback
        : finite(matrix[1], fallback - nodeTitleHeight) + nodeTitleHeight;
}
/** Measure a descendant center relative to a row before absolute positioning applies. */
function readCenterOffset(row, descendant) {
    const rowRect = row.getBoundingClientRect();
    const descendantRect = descendant.getBoundingClientRect();
    const measured = (descendantRect.top + descendantRect.height / 2 - rowRect.top) / readScale(row);
    return Number.isFinite(measured) ? measured : SOCKET_EDGE_INSET;
}
/** Measure an element's local height with an offset-height fallback. */
function readLocalHeight(scaleOwner, element) {
    const measured = element.getBoundingClientRect().height / readScale(scaleOwner);
    if (Number.isFinite(measured) && measured > 0)
        return measured;
    return Math.max(1, element.offsetHeight);
}
/** Resolve uniform canvas scale from one element's rendered and layout widths. */
function readScale(element) {
    const renderedWidth = element.getBoundingClientRect().width;
    const scale = element.offsetWidth > 0 ? renderedWidth / element.offsetWidth : 1;
    return Number.isFinite(scale) && scale > 0 ? scale : 1;
}
/** Read one finite value without propagating an invalid host constant. */
function finite(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) ? number : fallback;
}
/** Clamp a finite layout coordinate to an inclusive boundary. */
function clamp(value, minimum, maximum) {
    const finite = Number.isFinite(value) ? value : minimum;
    return Math.min(Math.max(finite, minimum), maximum);
}
