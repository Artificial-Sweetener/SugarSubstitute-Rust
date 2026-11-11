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

import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { ImageDimensions } from '../../execution/domain/WorkflowOutput.js';
import { canvasBatchLayout } from './CanvasBatchLayout.js';
import type { CanvasCommand } from '../domain/CanvasViewPort.js';

export interface CanvasOverviewCell {
  readonly key: string;
  readonly selection: CanvasSelection;
  readonly label: string;
  readonly item: WorkflowOutputItem | null;
  readonly preview: boolean;
  /** The scene a cell of the every-scene overview opens. */
  readonly scene?: string;
}

/** Open the scene of an every-scene overview cell; other cells select their own image. */
export function canvasOverviewSceneCommand(cell: CanvasOverviewCell): CanvasCommand | null {
  return cell.scene === undefined ? null : { type: 'select-scene', scene: cell.scene };
}

export interface CanvasOverviewWindow {
  readonly columns: number;
  readonly rowHeight: number;
  readonly offset: number;
  readonly totalHeight: number;
  readonly count: number;
  readonly cells: readonly CanvasOverviewCell[];
  readonly left: number;
  readonly width: number;
  readonly cellHeight: number;
  readonly gap: number;
  /** The scroll position the window was laid out for, clamped to its finite track. */
  readonly scroll: number;
}

/** Locate one laid-out cell in the overview's visible box, in CSS pixels. */
export interface CanvasOverviewPlacement {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  /** Horizontal offset centering a partial final packed row, already included in `x`. */
  readonly shift: number;
}

/**
 * Place each window cell exactly as the overview grid lays it out.
 *
 * Cells fill rows of equal-width columns separated by the window gap; a partial final row of a
 * packed batch is centered. Coordinates are relative to the overview's visible top-left, so a
 * partially scrolled row can start above zero.
 */
export function canvasOverviewPlacements(
  window: CanvasOverviewWindow,
): readonly CanvasOverviewPlacement[] {
  const columnWidth = (window.width - (window.columns - 1) * window.gap) / window.columns;
  const finalCount = window.count % window.columns;
  return Object.freeze(
    window.cells.map((_, index) => {
      const finalRow = window.count <= 8 && finalCount > 0 && index >= window.count - finalCount;
      const shift = finalRow
        ? ((window.columns - finalCount) * (window.width + window.gap)) / window.columns / 2
        : 0;
      return Object.freeze({
        x: window.left + (index % window.columns) * (columnWidth + window.gap) + shift,
        y:
          window.offset -
          window.scroll +
          Math.floor(index / window.columns) * (window.cellHeight + window.gap),
        width: columnWidth,
        height: window.cellHeight,
        shift,
      });
    }),
  );
}

/** Fit up to eight images together; bound larger batches to eight intersecting cells. */
export function canvasOverviewWindow(
  catalog: CanvasCatalogSnapshot,
  width: number,
  height: number,
  scrollTop: number,
  reference: ImageDimensions | null = null,
  previousColumns: number | null = null,
): CanvasOverviewWindow {
  if (![width, height, scrollTop].every((value) => Number.isFinite(value) && value >= 0))
    throw new RangeError('Canvas overview viewport is invalid.');
  const count = catalog.entries.reduce(
    (sum, entry) => sum + Math.max(entry.summary.batchCount, entry.preview ? 1 : 0),
    0,
  );
  if (!Number.isSafeInteger(count)) throw new RangeError('Canvas overview catalog is too large.');
  const dimensions = reference ??
    catalog.entries[0]?.items[0]?.dimensions ??
    // Use the layout policy's 512-pixel reference scale until an adapter knows the aspect.
    // A 1-pixel square makes the minimum two-image-pixel gutter larger than each cell.
    catalog.entries[0]?.preview?.dimensions ?? { width: 512, height: 512 };
  const packed =
    count > 0 && count <= 8 && width > 0 && height > 0
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
  const end =
    width > 0 && height > 0
      ? packed
        ? count
        : Math.min(count, Math.ceil((logicalScroll + height) / rowHeight) * columns)
      : first;
  const cells: CanvasOverviewCell[] = [];
  let preceding = 0;
  for (const entry of catalog.entries) {
    const batchCount = Math.max(entry.summary.batchCount, entry.preview ? 1 : 0);
    if (preceding >= end) break;
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
        cells.push(
          Object.freeze({
            key: JSON.stringify(
              entry.scene === undefined
                ? [selection.sourceId, index]
                : ['scene', entry.scene, selection.sourceId, index],
            ),
            selection,
            label:
              entry.scene === undefined
                ? `${entry.summary.label} · ${preview ? 'Source preview' : `${item?.mediaKind === 'model' ? '3D model' : item?.mediaKind === 'video' ? 'Video' : item?.mediaKind === 'animated' ? 'Animation' : 'Image'} ${index + 1}`}`
                : entry.summary.label,
            item,
            preview,
            ...(entry.scene === undefined ? {} : { scene: entry.scene }),
          }),
        );
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
