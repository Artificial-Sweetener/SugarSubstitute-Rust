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
/** Own linked and independent lightweight inspection state within each canvas view. */

import type { CanvasViewId } from '../domain/CanvasIdentity.js';
import type { CanvasSelection } from '../domain/CanvasViewState.js';
import {
  captureCanvasInspection,
  projectCanvasInspection,
  type CanvasInspection,
} from '../domain/CanvasInspection.js';
import {
  centeredCanvasCamera,
  type CanvasCamera,
  type CanvasCameraGeometry,
} from '../domain/CanvasCamera.js';

interface ViewInspection {
  generation: number;
  presentedRun: number | null;
  linked: boolean;
  shared: CanvasInspection | null;
  readonly individual: Map<string, CanvasInspection>;
}

export interface CanvasInspectionTarget {
  readonly previewLane: string | null;
  readonly selection: CanvasSelection;
  readonly camera: CanvasCamera;
  readonly geometry: CanvasCameraGeometry;
}

/** Bound independent camera references to current workflow slots rather than accumulating output history. */
export class CanvasInspectionStore {
  readonly #views = new Map<CanvasViewId, ViewInspection>();

  /** Reset replacement generations and prune slots removed from the current catalog. */
  synchronize(view: CanvasViewId, generation: number, slots: readonly CanvasSelection[]): void {
    let record = this.#views.get(view);
    if (!record || record.generation !== generation) {
      record = {
        generation,
        presentedRun: null,
        linked: true,
        shared: null,
        individual: new Map(),
      };
      this.#views.set(view, record);
    }
    const valid = new Set(slots.map(selectionKey));
    for (const slot of record.individual.keys())
      if (!valid.has(slot)) record.individual.delete(slot);
  }

  /** Retire previous-Run cameras only when a newer Run's pixels are actually presented. */
  presentRun(view: CanvasViewId, runOrder: number): boolean {
    const record = this.#require(view);
    if (record.presentedRun !== null && runOrder <= record.presentedRun) return false;
    record.presentedRun = runOrder;
    record.shared = null;
    record.individual.clear();
    return true;
  }

  /** Change link policy without overwriting saved independent slot cameras. */
  setLinked(view: CanvasViewId, linked: boolean, current: CanvasInspectionTarget | null): void {
    const record = this.#require(view);
    if (record.linked === linked) return;
    if (current && current.previewLane === null) {
      const inspection = captureCanvasInspection(current.camera, current.geometry);
      const key = selectionKey(current.selection);
      if (linked) record.shared = inspection;
      else if (!record.individual.has(key)) record.individual.set(key, inspection);
    }
    record.linked = linked;
  }

  /** Save final inspection without allowing preview presentation to replace it. */
  capture(
    view: CanvasViewId,
    slot: CanvasSelection,
    camera: CanvasCamera,
    geometry: CanvasCameraGeometry,
    previewLane: string | null,
  ): void {
    const record = this.#require(view);
    if (previewLane !== null) return;
    const inspection = captureCanvasInspection(camera, geometry);
    if (record.linked) record.shared = inspection;
    else record.individual.set(selectionKey(slot), inspection);
  }

  /** Fit new preview compositions independently of existing linked final inspection. */
  camera(
    view: CanvasViewId,
    slot: CanvasSelection,
    geometry: CanvasCameraGeometry,
    previewLane: string | null,
  ): CanvasCamera {
    const record = this.#require(view);
    if (previewLane !== null) return centeredCanvasCamera(geometry);
    const inspection = record.linked ? record.shared : record.individual.get(selectionKey(slot));
    return inspection
      ? projectCanvasInspection(inspection, geometry)
      : centeredCanvasCamera(geometry);
  }

  /** Release all image-relative state when its view closes. */
  remove(view: CanvasViewId): void {
    this.#views.delete(view);
  }

  /** End this integration without keeping workflow inspection state alive. */
  dispose(): void {
    this.#views.clear();
  }

  /** Require lifecycle enrollment before accepting camera state. */
  #require(view: CanvasViewId): ViewInspection {
    const record = this.#views.get(view);
    if (!record) throw new Error('Canvas inspection view is unavailable.');
    return record;
  }
}

/** Key independent slots without retaining superseded execution result identities. */
function selectionKey(selection: CanvasSelection): string {
  return JSON.stringify([selection.sourceId, selection.batchIndex]);
}
