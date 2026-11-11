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
/** Present packed batch images without owning image loading or result history. */

import type { CanvasOverviewCell } from '../../application/CanvasOverviewWindow.js';
import type { CanvasFrameClock } from '../../application/CanvasCameraTransition.js';
import type { CanvasCatalogSnapshot } from '../../domain/CanvasCatalog.js';
import type {
  CanvasImageSlotSnapshot,
  CanvasImageFrame,
} from '../../domain/CanvasImageResource.js';
import type { CanvasMediaTarget } from '../../domain/CanvasMedia.js';
import { CanvasOverviewGrid, type CanvasOverviewGridCell } from '../CanvasOverviewGrid.js';

/** Paint the shared grid by adopting already-admitted browser image handles. */
export class BrowserCanvasOverview {
  readonly element: HTMLDivElement;
  readonly #grid: CanvasOverviewGrid;
  #images: readonly CanvasImageSlotSnapshot<HTMLImageElement>[] = [];

  /** Bind explicit visible-interest and navigation collaborators without accessing the Comfy host. */
  constructor(
    parent: HTMLElement,
    frames: CanvasFrameClock,
    visible: (cells: readonly CanvasOverviewCell[]) => void,
    select: (cell: CanvasOverviewCell) => void,
  ) {
    this.#grid = new CanvasOverviewGrid(parent, {
      frames,
      visible,
      select,
      painter: {
        reference: () => this.#images.find((image) => image.frame)?.frame?.dimensions ?? null,
        paint: (cells) => this.#present(cells),
        clear: () => {
          this.#images = [];
        },
      },
    });
    this.element = this.#grid.element;
  }

  /** Refresh light catalog descriptors, retaining controls whose logical cells remain visible. */
  reconcile(catalog: CanvasCatalogSnapshot, active: boolean): void {
    this.#grid.reconcile(catalog, active);
  }

  /** Re-measure a settled allocation and restore interests after resource suspension. */
  resize(): void {
    this.#grid.render(true);
  }

  /** Project a pending source's visible cells without replacing the current gallery. */
  cells(catalog: CanvasCatalogSnapshot): readonly CanvasOverviewCell[] {
    return this.#grid.cells(catalog);
  }

  /** Attach resource-owned handles without copying pixels, URLs, or decode authority. */
  images(images: readonly CanvasImageSlotSnapshot<HTMLImageElement>[]): void {
    this.#images = images;
    this.#grid.render(false);
  }

  /** Qualify the exact rendered batch item without admitting latent previews or pending replacements. */
  draggable(target: EventTarget | null): CanvasImageFrame<HTMLImageElement> | null {
    if (!this.#grid.active) return null;
    for (const elements of this.#grid.cellElements) {
      const frame = this.#frame(elements);
      if (
        frame &&
        !frame.preview &&
        target === frame.handle &&
        frame.key === elements.cell.item?.id
      )
        return frame;
    }
    return null;
  }

  /** Pin context actions to the completed image actually painted in this visible batch cell. */
  contextTarget(target: EventTarget | null): CanvasMediaTarget | null {
    if (!this.#grid.active || !(target instanceof Node)) return null;
    for (const elements of this.#grid.cellElements) {
      const frame = this.#frame(elements);
      const item = elements.cell.item;
      if (
        item &&
        frame &&
        !frame.preview &&
        frame.key === item.id &&
        elements.button.contains(target)
      )
        return Object.freeze({ sourceId: elements.cell.selection.sourceId, itemId: item.id });
    }
    return null;
  }

  /** Stop finite scroll/resize work and detach this gallery's controls and image references. */
  dispose(): void {
    this.#grid.dispose();
  }

  /** Read the admitted frame for one visible cell's resource slot. */
  #frame(elements: CanvasOverviewGridCell): CanvasImageFrame<HTMLImageElement> | null {
    return this.#images.find((image) => image.slot === elements.cell.key)?.frame ?? null;
  }

  /** Preserve resource failure/budget explanations instead of pretending every cell has a thumbnail. */
  #present(cells: readonly CanvasOverviewGridCell[]): void {
    for (const elements of cells) {
      const snapshot = this.#images.find((image) => image.slot === elements.cell.key);
      const handle = snapshot?.frame?.handle;
      if (handle && elements.image.firstChild !== handle) {
        handle.className = 'sc-output-overview-thumbnail';
        handle.alt = '';
        elements.image.replaceChildren(handle);
      } else if (!handle) elements.image.replaceChildren();
      elements.status.textContent = handle
        ? ''
        : snapshot?.status.state === 'error' || snapshot?.status.state === 'budget'
          ? snapshot.status.message
          : elements.cell.preview
            ? ''
            : !elements.cell.item
              ? 'This batch image is unavailable.'
              : elements.cell.item.mediaKind !== 'image'
                ? 'Select to open this media.'
                : '';
    }
  }
}
