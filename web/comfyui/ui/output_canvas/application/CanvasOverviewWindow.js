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
/** Pack small batches and virtualize larger batches without expanding the complete catalog. */
import { canvasBatchLayout } from './CanvasBatchLayout.js';
/**
 * Place each window cell exactly as the overview grid lays it out.
 *
 * Cells fill rows of equal-width columns separated by the window gap; a partial final row of a
 * packed batch is centered. Coordinates are relative to the overview's visible top-left, so a
 * partially scrolled row can start above zero.
 */
export function canvasOverviewPlacements(window) {
    const columnWidth = (window.width - (window.columns - 1) * window.gap) / window.columns;
    const finalCount = window.count % window.columns;
    return Object.freeze(window.cells.map((_, index) => {
        const finalRow = window.count <= 8 && finalCount > 0 && index >= window.count - finalCount;
        const shift = finalRow
            ? ((window.columns - finalCount) * (window.width + window.gap)) / window.columns / 2
            : 0;
        return Object.freeze({
            x: window.left + (index % window.columns) * (columnWidth + window.gap) + shift,
            y: window.offset -
                window.scroll +
                Math.floor(index / window.columns) * (window.cellHeight + window.gap),
            width: columnWidth,
            height: window.cellHeight,
            shift,
        });
    }));
}
/** Fit up to eight images together; bound larger batches to eight intersecting cells. */
export function canvasOverviewWindow(catalog, width, height, scrollTop, reference = null, previousColumns = null) {
    if (![width, height, scrollTop].every((value) => Number.isFinite(value) && value >= 0))
        throw new RangeError('Canvas overview viewport is invalid.');
    const count = catalog.entries.reduce((sum, entry) => sum + Math.max(entry.summary.batchCount, entry.preview ? 1 : 0), 0);
    if (!Number.isSafeInteger(count))
        throw new RangeError('Canvas overview catalog is too large.');
    const dimensions = reference ??
        catalog.entries[0]?.items[0]?.dimensions ??
        catalog.entries[0]?.preview?.dimensions ?? { width: 1, height: 1 };
    const packed = count > 0 && count <= 8 && width > 0 && height > 0
        ? canvasBatchLayout(count, { width, height }, dimensions, previousColumns)
        : null;
    const columns = packed?.columns ?? (width >= 480 ? 2 : 1);
    const rowHeight = packed ? packed.cellHeight + packed.gap : Math.max(196, Math.ceil(height / 3));
    const logicalHeight = Math.ceil(count / columns) * rowHeight;
    if (!Number.isFinite(logicalHeight) || logicalHeight > Number.MAX_SAFE_INTEGER)
        throw new RangeError('Canvas overview extent is invalid.');
    // Browser scroll extents have finite pixel limits; map large logical catalogs onto a safe track.
    const totalHeight = Math.min(8_000_000, logicalHeight);
    const scroll = Math.min(scrollTop, Math.max(0, totalHeight - height));
    const track = Math.max(0, totalHeight - height);
    const logicalScroll = track > 0 ? (scroll * Math.max(0, logicalHeight - height)) / track : 0;
    const firstRow = Math.floor(logicalScroll / rowHeight);
    const first = firstRow * columns;
    const end = width > 0 && height > 0
        ? packed
            ? count
            : Math.min(count, Math.ceil((logicalScroll + height) / rowHeight) * columns)
        : first;
    const cells = [];
    let preceding = 0;
    for (const entry of catalog.entries) {
        const batchCount = Math.max(entry.summary.batchCount, entry.preview ? 1 : 0);
        if (preceding >= end)
            break;
        if (preceding + batchCount > first) {
            const start = Math.max(0, first - preceding);
            const stop = Math.min(batchCount, end - preceding);
            for (let index = start; index < stop; index++) {
                const item = entry.items.find((item) => item.batchIndex === index) ?? null;
                const preview = entry.preview !== null && index === 0;
                const selection = Object.freeze({
                    sourceId: preview ? entry.source.id : (item?.provenance.sourceId ?? entry.source.id),
                    batchIndex: index,
                });
                cells.push(Object.freeze({
                    key: JSON.stringify([selection.sourceId, index]),
                    selection,
                    label: `${entry.summary.label} · ${preview ? 'Source preview' : `${item?.mediaKind === 'model' ? '3D model' : item?.mediaKind === 'video' ? 'Video' : item?.mediaKind === 'animated' ? 'Animation' : 'Image'} ${index + 1}`}`,
                    item,
                    preview,
                }));
            }
        }
        preceding += batchCount;
    }
    return Object.freeze({
        columns,
        rowHeight,
        offset: packed?.top ?? scroll + firstRow * rowHeight - logicalScroll,
        totalHeight: packed ? height : totalHeight,
        count,
        cells: Object.freeze(cells),
        left: packed?.left ?? 0,
        width: packed?.width ?? width,
        cellHeight: packed?.cellHeight ?? rowHeight,
        gap: packed?.gap ?? 0,
        scroll,
    });
}
