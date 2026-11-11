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
/** Insert constructed Cube nodes with Sugar-owned catalog and history behavior. */

import type { ImportPayload } from '../import/PlacementPayload.js';
import type {
  BuiltCubeConstruction,
  ConstructedCube,
  CubeConstructionOptions,
  CubeConstructionService,
} from './CubeConstructionService.js';
import type { CubeNode } from './node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from './node/CubeNodeCatalog.js';
import { readInstanceId } from './node/CubeNodeCatalog.js';

export type CubePlacementOptions = CubeConstructionOptions;
export type CubeIdentity = import('./CubeConstructionService.js').CubeIdentity;

export interface CubePlacementHistory {
  beforeChange?(): void;
  afterChange?(): void;
  setDirtyCanvas?(foreground?: boolean, background?: boolean): void;
}

export interface BuiltCubePlacement extends BuiltCubeConstruction {
  recordHistory: boolean;
}

export interface PlacedCube extends ConstructedCube {
  node: CubeNode;
}

export interface CubePlacementGraph {
  add(node: CubeNode): void;
  remove?(node: CubeNode): void;
}

export interface CubePlacementBatchItem {
  payload: ImportPayload;
  options?: CubePlacementOptions;
}

export interface CubePlacementServiceOptions {
  construction: CubeConstructionService;
  graph: CubePlacementGraph;
  catalog: CubeNodeCatalog;
  history: CubePlacementHistory;
}

/** Own root-graph insertion, catalog registration, and history boundaries. */
export class CubePlacementService {
  readonly #construction: CubeConstructionService;
  readonly #graph: CubePlacementGraph;
  readonly #catalog: CubeNodeCatalog;
  readonly #history: CubePlacementHistory;

  /** Bind detached construction to the normal Sugar import insertion policy. */
  constructor(options: CubePlacementServiceOptions) {
    this.#construction = options.construction;
    this.#graph = options.graph;
    this.#catalog = options.catalog;
    this.#history = options.history;
  }

  /** Construct and insert one imported Cube definition. */
  place(payload: ImportPayload, options: CubePlacementOptions = {}): PlacedCube {
    return this.#withHistory(true, () =>
      this.#insert(this.#construction.construct(payload, options)),
    );
  }

  /** Construct and insert one prebuilt native definition. */
  placeBuilt(request: BuiltCubePlacement): PlacedCube {
    return this.#withHistory(request.recordHistory, () =>
      this.#insert(this.#construction.constructBuilt(request)),
    );
  }

  /** Atomically construct, insert, configure, and connect one workflow batch. */
  placeBatch(
    items: readonly CubePlacementBatchItem[],
    finalize: (placed: readonly PlacedCube[]) => void,
  ): readonly PlacedCube[] {
    const constructed: ConstructedCube[] = [];
    try {
      for (const item of items) {
        constructed.push(this.#construction.construct(item.payload, item.options));
      }
    } catch (error: unknown) {
      for (const item of constructed.reverse()) this.#construction.discard(item);
      throw error;
    }

    const inserted: PlacedCube[] = [];
    return this.#withHistory(true, () => {
      try {
        for (const item of constructed) {
          this.#graph.add(item.node);
          inserted.push(item);
          this.#catalog.add(item.node);
        }
        finalize(inserted);
        this.#history.setDirtyCanvas?.(true, true);
        return inserted;
      } catch (error: unknown) {
        this.#rollbackBatch(inserted, constructed);
        throw error;
      }
    });
  }

  /** Insert one already-constructed node through normal Sugar placement ownership. */
  #insert(constructed: ConstructedCube): PlacedCube {
    this.#graph.add(constructed.node);
    this.#catalog.add(constructed.node);
    this.#history.setDirtyCanvas?.(true, true);
    return constructed;
  }

  /** Remove every inserted node and detached definition after batch failure. */
  #rollbackBatch(inserted: readonly PlacedCube[], constructed: readonly ConstructedCube[]): void {
    for (const item of [...inserted].reverse()) {
      this.#catalog.remove(readInstanceId(item.node));
      this.#graph.remove?.(item.node);
    }
    for (const item of [...constructed].reverse()) this.#construction.discard(item);
  }

  /** Preserve one balanced host history boundary around a placement mutation. */
  #withHistory<T>(recordHistory: boolean, operation: () => T): T {
    if (recordHistory) this.#history.beforeChange?.();
    try {
      return operation();
    } finally {
      if (recordHistory) this.#history.afterChange?.();
    }
  }
}
