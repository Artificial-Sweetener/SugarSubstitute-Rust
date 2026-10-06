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
/** Keep reversible shared-field presentation separate from native widget eligibility and values. */
import type { ComfyWidget } from '../types/graph.js';

const hiddenWidgets = new WeakMap<ComfyWidget, NativeWidgetPresentation>();

/** Read transient presentation without excluding a widget from semantic discovery. */
export function isNativeWidgetPresented(widget: ComfyWidget): boolean {
  return !hiddenWidgets.has(widget);
}

/** Own one scope of widget visibility leases without altering serialization or native controls. */
export class NativeWidgetPresentation {
  readonly #hidden = new Set<ComfyWidget>();

  /** Apply the next resolved presentation and return whether native layout needs refreshing. */
  reconcile(widgets: ReadonlySet<ComfyWidget>): boolean {
    let changed = false;
    for (const widget of this.#hidden)
      if (!widgets.has(widget)) {
        if (hiddenWidgets.get(widget) === this) hiddenWidgets.delete(widget);
        this.#hidden.delete(widget);
        changed = true;
      }
    for (const widget of widgets)
      if (!this.#hidden.has(widget)) {
        const owner = hiddenWidgets.get(widget);
        if (owner && owner !== this)
          throw new Error('Native widget visibility has competing owners.');
        hiddenWidgets.set(widget, this);
        this.#hidden.add(widget);
        changed = true;
      }
    return changed;
  }

  /** Restore presentation immediately when a workflow or owning host is released. */
  dispose(): void {
    this.reconcile(new Set());
  }
}
