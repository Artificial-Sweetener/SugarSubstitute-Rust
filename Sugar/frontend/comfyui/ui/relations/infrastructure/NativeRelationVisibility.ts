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
/** Project shared-field presentation into both native renderers without changing eligibility. */
import type { ComfyWidget } from '../../types/graph.js';
import { isRecord } from '../../types/common.js';
import { NativeWidgetPresentation } from '../../surface/NativeWidgetPresentation.js';
import type { RelationSnapshot } from '../application/WorkflowRelations.js';
import type { NativeRelationInventory } from './NativeRelationInventory.js';

/** Hide fields represented by the toolbar or a linked node while retaining native value owners. */
export class NativeRelationVisibility {
  readonly #presentation = new NativeWidgetPresentation();
  #hiddenFields = new Map<string, string>();

  /** Reconcile resolved ownership rather than inspecting card DOM or workflow implementation leaves. */
  reconcile(snapshot: RelationSnapshot, inventory: NativeRelationInventory): readonly string[] {
    const hidden = new Set<ComfyWidget>();
    const fields = new Map<string, string>();
    for (const item of snapshot.resolved) {
      if (!item.override && !(item.source && snapshot.state.nodeLinks[item.field.nodeId])) continue;
      const owner = inventory.owners.get(item.field.id);
      if (!owner) continue;
      fields.set(item.field.id, item.field.nodeId);
      hidden.add(owner.widget);
      const faceWidget = owner.face.widgets?.find((widget) => widget.name === item.field.name);
      if (faceWidget) hidden.add(faceWidget);
      if (Array.isArray(owner.widget.linkedWidgets))
        for (const widget of owner.widget.linkedWidgets) if (isWidget(widget)) hidden.add(widget);
    }
    const changed = new Set<string>();
    for (const [id, nodeId] of this.#hiddenFields) if (!fields.has(id)) changed.add(nodeId);
    for (const [id, nodeId] of fields) if (!this.#hiddenFields.has(id)) changed.add(nodeId);
    this.#presentation.reconcile(hidden);
    this.#hiddenFields = fields;
    return [...changed];
  }

  /** Release all leases before a different workflow is configured. */
  dispose(): void {
    this.#presentation.dispose();
    this.#hiddenFields.clear();
  }
}

/** Validate linked native control identities before giving the renderer a visibility lease. */
function isWidget(value: unknown): value is ComfyWidget {
  return isRecord(value) && typeof value.name === 'string' && 'value' in value;
}
