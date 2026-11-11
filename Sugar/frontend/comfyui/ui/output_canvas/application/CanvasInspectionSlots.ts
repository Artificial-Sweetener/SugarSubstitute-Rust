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
/** Derive camera addresses from the existing result owner's exact scene membership. */
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import type { CanvasInspectionSlot } from '../domain/CanvasInspection.js';
import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';

/** Index current finals once so camera capture remains linear in retained scene membership. */
export function canvasInspectionCatalog(
  catalog: CanvasCatalogSnapshot,
): ReadonlyMap<string, CanvasInspectionSlot> {
  const sources = new Set(catalog.sources.map((source) => source.id));
  const slots = new Map<string, CanvasInspectionSlot>();
  const collect = (item: WorkflowOutputItem, scene?: readonly [string, string]): void => {
    if (!sources.has(item.provenance.sourceId)) return;
    slots.set(
      item.id,
      Object.freeze({
        sourceId: item.provenance.sourceId,
        batchIndex: item.batchIndex,
        ...(scene ? { scene } : {}),
      }),
    );
  };
  for (const output of catalog.result.outputs) for (const item of output.items) collect(item);
  const group = catalog.result.scenes;
  if (group)
    for (const scene of group.scenes) {
      const scope = Object.freeze([group.id, scene.key] as const);
      for (const output of scene.outputs) for (const item of output.items) collect(item, scope);
    }
  return slots;
}

/** Qualify one final without inferring scene membership from its source, filename or current tab. */
export function canvasInspectionSlot(
  selection: CanvasSelection,
  item: WorkflowOutputItem | null,
  catalog: CanvasCatalogSnapshot,
): CanvasInspectionSlot {
  return (
    (item ? canvasInspectionCatalog(catalog).get(item.id) : null) ?? Object.freeze({ ...selection })
  );
}

/** Retain every current scene's finals, including inactive scenes outside the projected catalog. */
export function canvasInspectionSlots(
  catalog: CanvasCatalogSnapshot,
): readonly CanvasInspectionSlot[] {
  return [...canvasInspectionCatalog(catalog).values()];
}
