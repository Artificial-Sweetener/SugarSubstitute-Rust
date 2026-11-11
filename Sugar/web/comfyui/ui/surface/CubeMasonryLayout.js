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
/** Own deterministic responsive masonry geometry for Cube surface cards. */
/** Match SugarSubstitute's responsive shortest-column masonry geometry. */
export function computeCubeMasonry(cards, options) {
    const gap = finiteNonNegative(options.gap);
    const availableWidth = Math.max(1, finiteNonNegative(options.availableWidth));
    const minimumColumnWidth = Math.max(1, finiteNonNegative(options.minimumColumnWidth));
    const widthColumnCapacity = Math.max(1, Math.floor((availableWidth + gap) / (minimumColumnWidth + gap)));
    const columnCount = widthColumnCapacity;
    const columnWidth = Math.floor((availableWidth - gap * (columnCount - 1)) / columnCount);
    const columnHeights = Array.from({ length: columnCount }, () => 0);
    const placements = [];
    for (const card of cards) {
        const height = finiteNonNegative(card.height);
        const columnSpan = normalizedColumnSpan(card.columnSpan, columnCount);
        const column = indexOfLowestSpanWindow(columnHeights, columnSpan);
        const y = maximum(columnHeights.slice(column, column + columnSpan));
        placements.push({
            id: card.id,
            column,
            x: column * (columnWidth + gap),
            y,
            width: columnWidth * columnSpan + gap * (columnSpan - 1),
            height,
        });
        for (let index = column; index < column + columnSpan; index += 1) {
            columnHeights[index] = y + height + gap;
        }
    }
    const occupiedHeights = columnHeights.map((height) => Math.max(0, height - gap));
    return {
        columnCount,
        columnWidth,
        height: Math.max(0, ...occupiedHeights),
        placements,
    };
}
/** Reconcile persisted card order with the nodes currently owned by the Cube graph. */
export function orderCubeSurfaceCards(persistedOrder, availableNodeIds) {
    const available = new Set(availableNodeIds);
    const ordered = [];
    const seen = new Set();
    for (const nodeId of [...persistedOrder, ...availableNodeIds]) {
        if (!available.has(nodeId) || seen.has(nodeId))
            continue;
        seen.add(nodeId);
        ordered.push(nodeId);
    }
    return ordered;
}
/** Return a safe finite non-negative geometry value. */
function finiteNonNegative(value) {
    return Number.isFinite(value) ? Math.max(0, value) : 0;
}
/** Clamp one requested masonry span to the currently available responsive columns. */
function normalizedColumnSpan(value, columnCount) {
    const candidate = Number.isFinite(value) ? Math.floor(value ?? 1) : 1;
    return Math.max(1, Math.min(columnCount, candidate));
}
/** Find the contiguous span whose tallest occupied column is lowest. */
function indexOfLowestSpanWindow(values, span) {
    let winner = 0;
    let winningHeight = Number.POSITIVE_INFINITY;
    for (let column = 0; column <= values.length - span; column += 1) {
        const height = maximum(values.slice(column, column + span));
        if (height < winningHeight) {
            winner = column;
            winningHeight = height;
        }
    }
    return winner;
}
/** Return a finite greatest value for a non-empty collection of column heights. */
function maximum(values) {
    return Math.max(0, ...values);
}
