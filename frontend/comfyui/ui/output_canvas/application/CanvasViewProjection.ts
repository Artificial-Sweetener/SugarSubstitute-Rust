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
/** Project one output view and keep its inspection slots aligned with the catalog. */

import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import { canvasDisplayPlan, type CanvasDisplayPlan } from './CanvasDisplayPlan.js';
import type { CanvasInspectionStore } from './CanvasInspectionStore.js';
import type { CanvasSelectionController } from './CanvasSelectionController.js';
import type { CanvasViewRegistry } from './CanvasViewRegistry.js';

/** Reuse the authoritative view, selection and inspection owners across surface implementations. */
export class CanvasViewProjection {
  /** Bind one view without owning its registry or retaining a workflow catalog. */
  constructor(
    private readonly view: CanvasViewId,
    private readonly views: CanvasViewRegistry,
    private readonly selection: CanvasSelectionController,
    private readonly inspections: CanvasInspectionStore,
  ) {}

  /** Resolve the current selection and enroll only the inspection slots this catalog can retain. */
  project(catalog: CanvasCatalogSnapshot): CanvasDisplayPlan {
    const plan = canvasDisplayPlan(
      this.views.read(this.view),
      catalog,
      this.selection.items(this.view, catalog),
      this.selection.preview(this.view, catalog),
    );
    const slots = catalog.entries.flatMap((entry) =>
      entry.items.map((item) => ({ sourceId: entry.source.id, batchIndex: item.batchIndex })),
    );
    for (const slot of [plan.primary, plan.secondary])
      if (slot?.sourcePresent) slots.push(slot.selection);
    this.inspections.synchronize(this.view, catalog.generation, slots);
    return plan;
  }
}
