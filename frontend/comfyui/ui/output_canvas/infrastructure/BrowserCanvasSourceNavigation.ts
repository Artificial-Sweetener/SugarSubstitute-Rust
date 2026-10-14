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
/** Coordinate a source-tab handoff without changing controls ahead of decoded pixels. */
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import { resolveCanvasReveal, type CanvasRevealTarget } from '../domain/CanvasReveal.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import { canvasOutputCatalog } from '../application/CanvasOutputCatalog.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import type { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import {
  canvasBatchGridCatalog,
  canvasSelectedItem,
} from '../application/CanvasContentSelection.js';
import type { CanvasOverviewCell } from '../application/CanvasOverviewWindow.js';
import type { BrowserCanvasResources } from './BrowserCanvasResources.js';

export interface BrowserCanvasSourceNavigationOptions {
  readonly resources: BrowserCanvasResources;
  readonly selection: CanvasSelectionController;
  /** Read the live attachment context again after asynchronous preparation. */
  read(): { readonly plan: CanvasDisplayPlan; readonly catalog: CanvasCatalogSnapshot } | null;
  /** Project target cells with the current overview's scroll and packing geometry. */
  cells(catalog: CanvasCatalogSnapshot): readonly CanvasOverviewCell[];
}

/** Keep source preparation fenced to the current workflow, run, and navigation. */
export class BrowserCanvasSourceNavigation {
  /** Bind the existing selection, layout, and resource authorities. */
  constructor(private readonly options: BrowserCanvasSourceNavigationOptions) {}

  /** Prepare a single final image even from a grid or comparison, fencing every delayed completion. */
  async reveal(target: CanvasRevealTarget): Promise<boolean> {
    const initial = this.options.read();
    if (!initial) return false;
    const item = resolveCanvasReveal(initial.catalog, target);
    if (!item) return false;
    const selection = { sourceId: target.sourceId, batchIndex: target.batchIndex };
    const inspection = canvasOutputCatalog(
      initial.catalog.result,
      initial.catalog.sources,
      selection,
    );
    const lease = this.options.resources.prepareSource(selection, inspection, null);
    const ready = lease ? await lease.ready : true;
    const current = this.options.read();
    if (
      !current ||
      current.plan.signature !== initial.plan.signature ||
      resolveCanvasReveal(current.catalog, target)?.id !== item.id
    ) {
      return false;
    }
    const apply = () =>
      this.options.selection.apply(current.plan.view.viewId, current.catalog, {
        type: 'reveal',
        target,
      });
    if (!ready) return lease?.unavailable ? apply() : false;
    return lease ? lease.commit(apply) : apply();
  }

  /** Prepare a completed source, then commit its normal selection policy synchronously. */
  async select(selection: CanvasSelection): Promise<boolean> {
    const initial = this.options.read();
    if (!initial) return false;
    const { plan, catalog } = initial;
    const apply = () =>
      this.options.selection.apply(plan.view.viewId, catalog, { type: 'select-source', selection });
    if (
      plan.view.comparison ||
      catalog.result.runStatus === 'running' ||
      catalog.result.runStatus === 'queued'
    ) {
      this.cancel();
      return apply();
    }
    const cells =
      plan.view.mode === 'overview'
        ? this.options.cells(canvasBatchGridCatalog(catalog, selection, false))
        : null;
    const lease = this.options.resources.prepareSource(selection, catalog, cells);
    const targets = cells
      ? JSON.stringify(cells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
      : canvasSelectedItem(catalog, selection)?.id;
    if (!lease) return apply();
    const ready = await lease.ready;
    const current = this.options.read();
    if (
      !current ||
      current.plan.signature !== plan.signature ||
      current.catalog.generation !== catalog.generation ||
      current.catalog.result.runOrder !== catalog.result.runOrder
    ) {
      this.cancel();
      return false;
    }
    const currentCells =
      plan.view.mode === 'overview'
        ? this.options.cells(canvasBatchGridCatalog(current.catalog, selection, false))
        : null;
    const currentTargets = currentCells
      ? JSON.stringify(currentCells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
      : canvasSelectedItem(current.catalog, selection)?.id;
    if (currentTargets !== targets) {
      this.cancel();
      return false;
    }
    if (!ready) return lease.unavailable ? apply() : false;
    return lease.commit(apply);
  }

  /** Cancel the pending source when another command or surface lifecycle takes precedence. */
  cancel(): void {
    this.options.resources.cancelPreparation();
  }
}
