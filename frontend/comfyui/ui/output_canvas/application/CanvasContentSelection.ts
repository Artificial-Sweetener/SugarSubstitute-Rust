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
/** Resolve semantic source/batch positions without retaining obsolete image revisions. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';

/** Resolve an exact slot without substituting a neighboring explicit batch image. */
export function canvasSelectedItem(
  catalog: CanvasCatalogSnapshot,
  selection: CanvasSelection | null,
): WorkflowOutputItem | null {
  if (!selection) return null;
  return (
    catalog.entries
      .find((entry) => entry.source.id === selection.sourceId)
      ?.items.find((item) => item.batchIndex === selection.batchIndex) ?? null
  );
}

/** Retain batch context for source navigation and fall back to a real available image. */
export function canvasAvailableSelection(
  catalog: CanvasCatalogSnapshot,
  selection: CanvasSelection,
): CanvasSelection {
  const items =
    catalog.entries.find((entry) => entry.source.id === selection.sourceId)?.items ?? [];
  const item = items.find((item) => item.batchIndex === selection.batchIndex) ?? items[0];
  return item ? canvasItemSelection(item) : selection;
}

/** Translate one final identity into the view selection shared by every renderer. */
export function canvasItemSelection(item: WorkflowOutputItem): CanvasSelection {
  return { sourceId: item.provenance.sourceId, batchIndex: item.batchIndex };
}

/**
 * Project a manual source grid or the progressively replaced automatic batch.
 *
 * The every-scene overview is already one cell per scene, so it is its own grid.
 */
export function canvasBatchGridCatalog(
  catalog: CanvasCatalogSnapshot,
  selection: CanvasSelection | null,
  automatic = false,
): CanvasCatalogSnapshot {
  if (catalog.sceneOverview) return catalog;
  const active = catalog.entries.find((entry) => entry.source.id === selection?.sourceId);
  if (!active) return Object.freeze({ ...catalog, entries: Object.freeze([]) });
  if (!automatic) return Object.freeze({ ...catalog, entries: Object.freeze([active]) });
  const slots = new Map<number, WorkflowOutputItem>();
  for (const entry of catalog.entries) {
    for (const item of entry.items) slots.set(item.batchIndex, item);
    if (entry === active) break;
  }
  // Comfy's source preview has no batch coordinate; its transient first tile is never a final.
  if (active.preview) slots.delete(0);
  const items = [...slots.values()].sort((a, b) => a.batchIndex - b.batchIndex);
  const batchCount = Math.max(active.preview ? 1 : 0, ...items.map((item) => item.batchIndex + 1));
  const entry = Object.freeze({
    ...active,
    items: Object.freeze(items),
    summary: Object.freeze({ ...active.summary, batchCount }),
  });
  return Object.freeze({ ...catalog, entries: Object.freeze([entry]) });
}
