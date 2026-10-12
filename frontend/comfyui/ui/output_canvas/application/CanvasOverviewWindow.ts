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
/** Project a bounded visible gallery window without expanding the workflow's complete batch catalog. */

import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';

export interface CanvasOverviewCell {
  readonly key: string;
  readonly selection: CanvasSelection;
  readonly label: string;
  readonly item: WorkflowOutputItem | null;
  readonly preview: boolean;
}

export interface CanvasOverviewWindow {
  readonly columns: number;
  readonly rowHeight: number;
  readonly offset: number;
  readonly totalHeight: number;
  readonly count: number;
  readonly cells: readonly CanvasOverviewCell[];
}

/** Limit visible cells to eight by using at most two columns and at most four intersecting rows. */
export function canvasOverviewWindow(
  catalog: CanvasCatalogSnapshot,
  width: number,
  height: number,
  scrollTop: number,
): CanvasOverviewWindow {
  if (![width, height, scrollTop].every((value) => Number.isFinite(value) && value >= 0))
    throw new RangeError('Canvas overview viewport is invalid.');
  const columns = width >= 480 ? 2 : 1;
  const rowHeight = Math.max(196, Math.ceil(height / 3));
  const count = catalog.entries.reduce(
    (sum, entry) => sum + Math.max(entry.summary.batchCount, entry.preview ? 1 : 0),
    0,
  );
  if (!Number.isSafeInteger(count)) throw new RangeError('Canvas overview catalog is too large.');
  const logicalHeight = Math.ceil(count / columns) * rowHeight;
  if (!Number.isSafeInteger(logicalHeight))
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
      ? Math.min(count, Math.ceil((logicalScroll + height) / rowHeight) * columns)
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
        const selection = Object.freeze({ sourceId: entry.source.id, batchIndex: index });
        const preview = entry.summary.batchCount === 0 && entry.preview !== null;
        cells.push(
          Object.freeze({
            key: JSON.stringify([selection.sourceId, index]),
            selection,
            label: `${entry.summary.label} · ${preview ? 'Source preview' : `Image ${index + 1}`}`,
            item: entry.items.find((item) => item.batchIndex === index) ?? null,
            preview,
          }),
        );
      }
    }
    preceding += batchCount;
  }
  return Object.freeze({
    columns,
    rowHeight,
    offset: scroll + firstRow * rowHeight - logicalScroll,
    totalHeight,
    count,
    cells: Object.freeze(cells),
  });
}
