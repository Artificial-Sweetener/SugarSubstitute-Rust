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
/** Coordinate prepared Cube payload insertion at the typed Comfy host boundary. */

import { planPlacementGeometry } from '../geometry/PlacementGeometryPlanner.js';
import {
  resolveRendererGeometryPolicy,
  type NodeRenderer,
  type RendererGeometryHost,
} from '../geometry/RendererGeometryPolicy.js';
import type { ImportResult } from './CubeImportTypes.js';
import { readImportPayload, type ImportPayload } from './PlacementPayload.js';
import type { GraphId, NumericVector } from '../types/graph.js';

interface PreparedImportLiteGraph extends RendererGeometryHost {
  createNode?: unknown;
}

interface PreparedImportRuntime {
  registerSubgraphs(payload: ImportPayload): string[];
  placement: {
    place(
      payload: ImportPayload,
      options?: { instanceAlias?: string },
    ): {
      node: { id: GraphId; pos: NumericVector; size: NumericVector };
      warnings: string[];
      internalNodeCount: number;
    };
  };
}

export interface CubePreparedImportDependencies {
  getGraph(): object | null | undefined;
  getLiteGraph(): PreparedImportLiteGraph | null | undefined;
  getNodeRenderer(): NodeRenderer | undefined;
  getRuntime(): PreparedImportRuntime;
  readErrorMessage(error: unknown): string;
}

export interface PreparedImportOptions {
  instanceAlias?: string;
}

/** Own validation, geometry policy, registration, and placement for one prepared import. */
export class CubePreparedImportService {
  readonly #dependencies: CubePreparedImportDependencies;

  /** Bind the dynamic Comfy surfaces behind a focused typed protocol. */
  constructor(dependencies: CubePreparedImportDependencies) {
    this.#dependencies = dependencies;
  }

  /** Insert one prepared Cube as a native root node with internal graph children. */
  apply(payloadValue: unknown, options: PreparedImportOptions = {}): ImportResult {
    const result = emptyImportResult();
    const parsedPayload = readImportPayload(payloadValue);
    if (!parsedPayload) {
      result.message = 'Importer payload missing';
      return result;
    }
    if (!this.#dependencies.getGraph()) {
      result.message = 'Graph unavailable';
      return result;
    }
    const liteGraph = this.#dependencies.getLiteGraph();
    if (!liteGraph || typeof liteGraph.createNode !== 'function') {
      result.message = 'LiteGraph unavailable';
      return result;
    }
    const geometryPolicy = resolveRendererGeometryPolicy(
      liteGraph,
      this.#dependencies.getNodeRenderer(),
    );
    const payload = planPlacementGeometry(parsedPayload, geometryPolicy);
    if (!payload.cube) {
      result.message = 'Importer payload is missing Cube identity';
      return result;
    }

    const runtime = this.#dependencies.getRuntime();
    result.warnings.push(...runtime.registerSubgraphs(payload));
    try {
      const placed = runtime.placement.place(payload, {
        ...(options.instanceAlias ? { instanceAlias: options.instanceAlias } : {}),
      });
      result.success = true;
      result.primaryNodeId = placed.node.id;
      result.warnings.push(...placed.warnings);
      result.summary = `cube 1, internal nodes ${String(placed.internalNodeCount)}`;
      const x = Number(placed.node.pos[0]) || 0;
      const y = Number(placed.node.pos[1]) || 0;
      const width = Number(placed.node.size[0]) || 0;
      const height = Number(placed.node.size[1]) || 0;
      result.bounds = {
        minX: x,
        minY: y,
        maxX: x + width,
        maxY: y + height,
      };
      return result;
    } catch (error: unknown) {
      const message = this.#dependencies.readErrorMessage(error);
      result.message = message;
      result.warnings.push(`Cube placement failed: ${message}`);
      return result;
    }
  }
}

/** Create the stable importer result contract before dynamic host work begins. */
function emptyImportResult(): ImportResult {
  return {
    success: false,
    summary: '',
    message: '',
    warnings: [],
    missingTypes: [],
    nodesAdded: 0,
    markersAdded: 0,
    connectionsMade: 0,
    primaryNodeId: null,
    bounds: null,
  };
}
