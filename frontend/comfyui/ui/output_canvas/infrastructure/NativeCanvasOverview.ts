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
/** Lay out the shared overview grid over cells that the native host paints. */

import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceOverviewCell,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import { fittingCanvasSurfaceOverviewCells } from '../../core/embedded/CanvasRuntime.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasFrameClock } from '../application/CanvasCameraTransition.js';
import type {
  CanvasOverviewCell,
  CanvasOverviewPlacement,
} from '../application/CanvasOverviewWindow.js';
import { canvasBatchGridCatalog } from '../application/CanvasContentSelection.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type { CanvasSurfaceGeometry } from '../domain/CanvasViewPort.js';
import type { CanvasViewState } from '../domain/CanvasViewState.js';
import {
  CanvasOverviewGrid,
  type CanvasOverviewBounds,
  type CanvasOverviewGridCell,
} from '../presentation/CanvasOverviewGrid.js';
import {
  CanvasOutputDrag,
  type CanvasOutputDragPayload,
  type CanvasOutputDragSubject,
} from '../presentation/CanvasOutputDrag.js';

/** The schema's overview-cell result identity ceiling, in Unicode code points. */
const MAX_OVERVIEW_ITEM_ID = 1024;
const FORMAT_EXTENSIONS: Readonly<
  Record<CanvasSurfaceCapabilities['imageFormats'][number], readonly string[]>
> = { png: ['.png'], jpeg: ['.jpg', '.jpeg'], webp: ['.webp'] };

/** Collaborators supplied by the native attachment that owns this overview. */
export interface NativeCanvasOverviewOptions {
  readonly frames: CanvasFrameClock;
  readonly address: CanvasAttachmentAddress;
  readonly capabilities: CanvasSurfaceCapabilities;
  /** The latest surface allocation; painted rects are relative to its origin. */
  geometry(): CanvasSurfaceGeometry | null;
  select(cell: CanvasOverviewCell): void;
  /** Notify the attachment that the natively painted cells changed. */
  changed(): void;
  /** Fence the fixed attachment before outbound drags. */
  accepts(): boolean;
  /** Prepare one painted image for the page's outbound drag. */
  drag?(item: WorkflowOutputItem, signal: AbortSignal): Promise<CanvasOutputDragPayload | null>;
  report(error: unknown): void;
}

/**
 * Report whether the native surface can paint a saved or temporary result in a negotiated format.
 *
 * The host admits Comfy result storage independently of editable input documents. Other media
 * stays in the grid as selectable controls instead of failing the whole grid.
 */
function paintable(item: WorkflowOutputItem, capabilities: CanvasSurfaceCapabilities): boolean {
  const filename = item.file.filename.toLowerCase();
  return (
    item.mediaKind === 'image' &&
    (item.file.type === 'output' || item.file.type === 'temp') &&
    Array.from(item.id).length <= MAX_OVERVIEW_ITEM_ID &&
    capabilities.imageFormats.some((format) =>
      FORMAT_EXTENSIONS[format].some((extension) => filename.endsWith(extension)),
    )
  );
}

/**
 * Express a laid-out cell's full rect relative to the surface origin.
 *
 * The host fits each image into its whole rect and clips to the viewport, so a partially
 * scrolled cell keeps its complete rect, including a negative origin; a cell entirely outside
 * the viewport is not painted.
 */
function surfaceRect(
  placement: CanvasOverviewPlacement,
  bounds: CanvasOverviewBounds,
  geometry: CanvasSurfaceGeometry,
): CanvasSurfaceOverviewCell['rect'] | null {
  const x = bounds.left - geometry.origin.x + placement.x;
  const y = bounds.top - geometry.origin.y + placement.y;
  const { width, height } = placement;
  const visible =
    x < geometry.viewport.width && y < geometry.viewport.height && x + width > 0 && y + height > 0;
  return visible &&
    [x, y, width, height].every((value) => Number.isFinite(value) && Math.abs(value) <= 1e6) &&
    width > 0 &&
    height > 0
    ? Object.freeze({ x, y, width, height })
    : null;
}

/** Keep the shared grid's controls while the host paints its admitted cells beneath them. */
export class NativeCanvasOverview {
  readonly element: HTMLDivElement;
  readonly #grid: CanvasOverviewGrid;
  readonly #drag: CanvasOutputDrag;
  #cells: readonly CanvasSurfaceOverviewCell[] = Object.freeze([]);
  #key = '[]';

  /** Mount the grid without decoding images or constructing browser image resources. */
  constructor(
    parent: HTMLElement,
    private readonly options: NativeCanvasOverviewOptions,
  ) {
    this.#grid = new CanvasOverviewGrid(parent, {
      frames: options.frames,
      select: (cell) => options.select(cell),
      painter: {
        reference: () => null,
        paint: (cells, placements, bounds) => this.#paint(cells, placements, bounds),
        clear: () => this.#publish(Object.freeze([])),
      },
    });
    this.element = this.#grid.element;
    // The host routes overview input to the page, so painted cells reuse the page's drag source.
    this.#drag = new CanvasOutputDrag(this.element, {
      resolve: (target) => this.#dragSubject(target),
      prepare: (key, signal) => {
        const item = this.#paintedItem(key);
        return item && options.drag ? options.drag(item, signal) : Promise.resolve(null);
      },
      accepts: () => this.#grid.active && options.accepts() && !!options.drag,
      report: (error) => options.report(error),
    });
  }

  /** Read the cells the host should currently paint, in grid order. */
  get cells(): readonly CanvasSurfaceOverviewCell[] {
    return this.#cells;
  }

  /**
   * Present the view's batch grid as it appears without live previews.
   *
   * Native previews do not stream while the overview is shown, so the grid is projected from
   * finals only and each first cell is the latest final, as the browser grid shows when no
   * preview is active.
   */
  reconcile(catalog: CanvasCatalogSnapshot, view: CanvasViewState, active: boolean): void {
    const finals = Object.freeze({
      ...catalog,
      entries: Object.freeze(
        catalog.entries.map((entry) =>
          entry.preview ? Object.freeze({ ...entry, preview: null }) : entry,
        ),
      ),
    });
    this.#grid.reconcile(
      canvasBatchGridCatalog(finals, view.selection, view.follow === 'auto'),
      active,
    );
    this.#drag.reconcile();
  }

  /** Discard a prepared drag after shared transfer preferences change. */
  invalidateDrag(): void {
    this.#drag.invalidate();
  }

  /** Re-measure after the surface allocation moves or resizes. */
  resize(): void {
    this.#grid.render(false);
  }

  /** Detach the grid's controls and release any prepared drag. */
  dispose(): void {
    this.#drag.dispose();
    this.#grid.dispose();
  }

  /** Resolve a pointer target to its cell only while the host paints that cell's image. */
  #dragSubject(target: EventTarget | null): CanvasOutputDragSubject | null {
    if (!(target instanceof Node)) return null;
    for (const { cell, button } of this.#grid.cellElements) {
      const item = cell.item;
      if (item && button.contains(target) && this.#paintedItem(item.id))
        return { key: item.id, element: button };
    }
    return null;
  }

  /** Find a currently painted image cell's item by identity. */
  #paintedItem(id: string): WorkflowOutputItem | null {
    if (!this.#cells.some((cell) => cell.itemId === id)) return null;
    const item = this.#grid.cellElements.find(({ cell }) => cell.item?.id === id)?.cell.item;
    return item?.mediaKind === 'image' ? item : null;
  }

  /** Admit paintable cells, explain the others, and publish only a changed layout. */
  #paint(
    cells: readonly CanvasOverviewGridCell[],
    placements: readonly CanvasOverviewPlacement[],
    bounds: CanvasOverviewBounds,
  ): void {
    const geometry = this.options.geometry();
    const candidates: CanvasSurfaceOverviewCell[] = [];
    const identities = new Set<string>();
    for (const [index, { cell }] of cells.entries()) {
      const placement = placements[index];
      const item = cell.item;
      if (!geometry || !placement || !item || identities.has(item.id)) continue;
      if (!paintable(item, this.options.capabilities)) continue;
      const rect = surfaceRect(placement, bounds, geometry);
      if (!rect) continue;
      identities.add(item.id);
      candidates.push(Object.freeze({ itemId: item.id, file: item.file, rect }));
    }
    const painted = fittingCanvasSurfaceOverviewCells(this.options.address, candidates);
    const shown = new Set(painted.map((cell) => cell.itemId));
    for (const { cell, status } of cells) {
      const item = cell.item;
      status.textContent =
        item && shown.has(item.id)
          ? ''
          : !item
            ? 'This batch image is unavailable.'
            : item.mediaKind !== 'image'
              ? 'Select to open this media.'
              : 'Select to view this image.';
    }
    this.#publish(Object.freeze(painted));
  }

  /** Notify the attachment only when the painted layout actually changed. */
  #publish(cells: readonly CanvasSurfaceOverviewCell[]): void {
    const key = JSON.stringify(cells);
    if (key === this.#key) return;
    this.#key = key;
    this.#cells = cells;
    this.#drag.reconcile();
    this.options.changed();
  }
}
