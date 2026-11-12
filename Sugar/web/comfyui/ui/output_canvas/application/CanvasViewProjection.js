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
import { canvasDisplayPlan } from './CanvasDisplayPlan.js';
import { canvasInspectionSlots } from './CanvasInspectionSlots.js';
/** Reuse the authoritative view, selection and inspection owners across surface implementations. */
export class CanvasViewProjection {
    view;
    views;
    selection;
    inspections;
    /** Bind one view without owning its registry or retaining a workflow catalog. */
    constructor(view, views, selection, inspections) {
        this.view = view;
        this.views = views;
        this.selection = selection;
        this.inspections = inspections;
    }
    /** Preserve inspections for retained finals while pruning slots outside current workflow sources. */
    project(catalog) {
        const plan = canvasDisplayPlan(this.views.read(this.view), catalog, this.selection.items(this.view, catalog), this.selection.preview(this.view, catalog));
        const slots = [...canvasInspectionSlots(catalog)];
        for (const slot of [plan.primary, plan.secondary])
            if (slot?.sourcePresent)
                slots.push(slot.selection);
        this.inspections.synchronize(this.view, catalog.generation, slots);
        return plan;
    }
}
