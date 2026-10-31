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

/** Pair a displayed final slot with its image-relative inspection, independent of any renderer. */
export interface CanvasInspectionSample {
  readonly selection: CanvasSelection;
  readonly inspection: CanvasInspection;
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
    if (!this.retires(view, runOrder)) return false;
    record.presentedRun = runOrder;
    record.shared = null;
    record.individual.clear();
    return true;
  }

  /** Report whether presenting this Run's final would retire saved cameras, without retiring them. */
  retires(view: CanvasViewId, runOrder: number): boolean {
    const record = this.#require(view);
    return record.presentedRun === null || runOrder > record.presentedRun;
  }

  /** Change link policy without overwriting saved independent slot cameras. */
  setLinked(view: CanvasViewId, linked: boolean, current: CanvasInspectionSample | null): void {
    const record = this.#require(view);
    if (record.linked === linked) return;
    if (current) {
      const key = selectionKey(current.selection);
      if (linked) record.shared = current.inspection;
      else if (!record.individual.has(key)) record.individual.set(key, current.inspection);
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
    if (previewLane !== null) {
      this.#require(view);
      return;
    }
    this.record(view, slot, captureCanvasInspection(camera, geometry));
  }

  /** Save one final slot's settled inspection under the view's current link policy. */
  record(view: CanvasViewId, slot: CanvasSelection, inspection: CanvasInspection): void {
    const record = this.#require(view);
    if (record.linked) record.shared = inspection;
    else record.individual.set(selectionKey(slot), inspection);
  }

  /** Read the inspection a final in this slot restores, or null when it starts at Fit. */
  inspection(view: CanvasViewId, slot: CanvasSelection): CanvasInspection | null {
    const record = this.#require(view);
    return (record.linked ? record.shared : record.individual.get(selectionKey(slot))) ?? null;
  }

  /** Fit new preview compositions independently of existing linked final inspection. */
  camera(
    view: CanvasViewId,
    slot: CanvasSelection,
    geometry: CanvasCameraGeometry,
    previewLane: string | null,
  ): CanvasCamera {
    const inspection = this.inspection(view, slot);
    if (previewLane !== null) return centeredCanvasCamera(geometry);
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
