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
/** Coordinate source handoffs without advancing controls ahead of prepared presentation. */
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import { resolveCanvasReveal, type CanvasRevealTarget } from '../domain/CanvasReveal.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasCommandScope } from '../domain/CanvasViewPort.js';
import { canvasOutputCatalog } from './CanvasOutputCatalog.js';
import type { CanvasDisplayPlan } from './CanvasDisplayPlan.js';
import type { CanvasSelectionController } from './CanvasSelectionController.js';
import { canvasBatchGridCatalog, canvasSelectedItem } from './CanvasContentSelection.js';
import type { CanvasOverviewCell } from './CanvasOverviewWindow.js';

/** Own one staged source until the surface adopts or retires it. */
export interface CanvasSourcePreparationLease {
  readonly ready: Promise<boolean>;
  readonly unavailable: boolean;
  /** Adopt prepared presentation while the shared selection is updated. */
  commit(apply: () => boolean): boolean;
}

/** Supply presentation readiness without exposing renderer-owned resources. */
export interface CanvasSourcePreparation {
  /** Retire the previous stage, then prepare this source or permit an immediate selection. */
  prepareSource(
    selection: CanvasSelection,
    catalog: CanvasCatalogSnapshot,
    cells: readonly CanvasOverviewCell[] | null,
    scope: CanvasCommandScope,
  ): CanvasSourcePreparationLease | null;
  /** Retire any pending handoff before another interaction takes ownership. */
  cancelPreparation(): void;
}

export interface CanvasSourceNavigationOptions {
  readonly resources: CanvasSourcePreparation;
  readonly selection: CanvasSelectionController;
  /** Read the live attachment context again after asynchronous preparation. */
  read(): { readonly plan: CanvasDisplayPlan; readonly catalog: CanvasCatalogSnapshot } | null;
  /** Project target cells with the current overview's scroll and packing geometry. */
  cells(catalog: CanvasCatalogSnapshot): readonly CanvasOverviewCell[];
}

/** Keep source preparation fenced to the current workflow, run, and navigation. */
export class CanvasSourceNavigation {
  #intent: object | null = null;

  /** Bind the existing selection, layout, and resource authorities. */
  constructor(private readonly options: CanvasSourceNavigationOptions) {}

  /** Prepare a single final image even from a grid or comparison, fencing every delayed completion. */
  async reveal(target: CanvasRevealTarget, scope: CanvasCommandScope): Promise<boolean> {
    if (!scope.current()) return false;
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
    const intent = {};
    this.#intent = intent;
    const lease = this.#prepare(intent, selection, inspection, null, scope);
    const ready = lease ? await this.#ready(intent, lease) : true;
    if (!this.#current(intent, scope)) return false;
    const current = this.options.read();
    if (
      !current ||
      current.plan.signature !== initial.plan.signature ||
      resolveCanvasReveal(current.catalog, target)?.id !== item.id
    ) {
      if (this.#intent === intent) this.cancel();
      return false;
    }
    const apply = () =>
      scope.current() &&
      this.options.selection.apply(current.plan.view.viewId, current.catalog, {
        type: 'reveal',
        target,
      });
    if (!this.#current(intent, scope)) return false;
    this.#intent = null;
    if (!ready) return lease?.unavailable ? apply() : false;
    return lease ? lease.commit(apply) : apply();
  }

  /** Prepare a completed source, then commit its normal selection policy synchronously. */
  async select(selection: CanvasSelection, scope: CanvasCommandScope): Promise<boolean> {
    if (!scope.current()) return false;
    const initial = this.options.read();
    if (!initial) return false;
    const { plan, catalog } = initial;
    const apply = () =>
      scope.current() &&
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
    const intent = {};
    this.#intent = intent;
    const lease = this.#prepare(intent, selection, catalog, cells, scope);
    const targets = cells
      ? JSON.stringify(cells.map((cell) => [cell.key, cell.item?.id, cell.preview]))
      : canvasSelectedItem(catalog, selection)?.id;
    if (!this.#current(intent, scope)) return false;
    if (!lease) {
      this.#intent = null;
      return apply();
    }
    const ready = await this.#ready(intent, lease);
    if (!this.#current(intent, scope)) return false;
    const current = this.options.read();
    if (
      !current ||
      current.plan.signature !== plan.signature ||
      current.catalog.generation !== catalog.generation ||
      current.catalog.result.runOrder !== catalog.result.runOrder
    ) {
      if (this.#intent === intent) this.cancel();
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
      if (this.#intent === intent) this.cancel();
      return false;
    }
    if (!this.#current(intent, scope)) return false;
    this.#intent = null;
    if (!ready) return lease.unavailable ? apply() : false;
    return lease.commit(apply);
  }

  /** Cancel the pending source when another command or surface lifecycle takes precedence. */
  cancel(): void {
    this.#intent = null;
    this.options.resources.cancelPreparation();
  }

  /** Retire only this still-owned preparation when its originating command loses authority. */
  #current(intent: object, scope: CanvasCommandScope): boolean {
    if (this.#intent !== intent) return false;
    if (scope.current() && this.#intent === intent) return true;
    if (this.#intent === intent) this.cancel();
    return false;
  }

  /** Retire a failed surface admission only if this request still owns it. */
  #prepare(
    intent: object,
    selection: CanvasSelection,
    catalog: CanvasCatalogSnapshot,
    cells: readonly CanvasOverviewCell[] | null,
    scope: CanvasCommandScope,
  ): CanvasSourcePreparationLease | null {
    try {
      return this.options.resources.prepareSource(selection, catalog, cells, scope);
    } catch (error: unknown) {
      return this.#fail(intent, error);
    }
  }

  /** Preserve a surface failure while retiring only its own still-current stage. */
  async #ready(intent: object, lease: CanvasSourcePreparationLease): Promise<boolean> {
    try {
      return await lease.ready;
    } catch (error: unknown) {
      return this.#fail(intent, error);
    }
  }

  /** Keep both the preparation and retirement failures observable when either occurs. */
  #fail(intent: object, error: unknown): never {
    if (this.#intent === intent) {
      try {
        this.cancel();
      } catch (retirement: unknown) {
        throw new AggregateError(
          [error, retirement],
          'Canvas source preparation retirement failed.',
        );
      }
    }
    throw error;
  }
}
