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
/** Retain executed Cube media across graph-bound runtime reconstruction. */

import type { CubePreviewSnapshot } from './CubePreviewModel.js';

interface RetainedPreviewSnapshot {
  outputSignature: string;
  snapshot: CubePreviewSnapshot;
}

/** Own last-known Cube media within the lifetime of one root graph object. */
export class CubePreviewRetentionStore {
  readonly #byRootGraph = new WeakMap<object, Map<string, RetainedPreviewSnapshot>>();

  /** Retain one media-bearing snapshot for a stable Cube instance. */
  retain(
    rootGraph: object,
    instanceId: string,
    outputSignature: string,
    snapshot: CubePreviewSnapshot,
  ): void {
    const retained = this.#byRootGraph.get(rootGraph) ?? new Map();
    retained.set(instanceId, { outputSignature, snapshot });
    this.#byRootGraph.set(rootGraph, retained);
  }

  /** Read retained media only when the Cube still exposes the same outputs. */
  read(rootGraph: object, instanceId: string, outputSignature: string): CubePreviewSnapshot | null {
    const retained = this.#byRootGraph.get(rootGraph)?.get(instanceId);
    return retained?.outputSignature === outputSignature ? retained.snapshot : null;
  }
}
