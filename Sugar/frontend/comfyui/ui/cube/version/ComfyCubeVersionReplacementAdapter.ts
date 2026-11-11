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
/** Replace one live Cube node while preserving compatible external graph connections. */

import type { CubePlacementHistory } from '../CubePlacementService.js';
import type { CubeNode } from '../node/ComfyCubeNodeFactory.js';
import { readInstanceId, type CubeNodeCatalog } from '../node/CubeNodeCatalog.js';
import type {
  NativeCubeConnection,
  NativeConnectionGraph,
} from '../graph/NativeCubeConnections.js';
import { readNativeCubeConnection } from '../graph/NativeCubeConnections.js';
import type { CubeVersionBoundaryMap } from './CubeVersionBoundaryMap.js';

export interface CubeReplacementGraph extends NativeConnectionGraph {
  add(node: CubeNode): void;
  remove(node: CubeNode): void;
}

export interface CubeReplacementSelection {
  replace(source: CubeNode, target: CubeNode): void;
}

export interface CubeVersionReplacementRequest {
  source: CubeNode;
  target: CubeNode;
  sourceBoundaries: CubeVersionBoundaryMap;
  targetBoundaries: CubeVersionBoundaryMap;
}

/** Own the single history boundary, rollback, catalog, selection, and reconnection mutation. */
export class ComfyCubeVersionReplacementAdapter {
  readonly #graph: CubeReplacementGraph;
  readonly #catalog: CubeNodeCatalog;
  readonly #history: CubePlacementHistory;
  readonly #selection: CubeReplacementSelection;
  readonly #logger: Pick<Console, 'error'>;

  /** Bind graph mutation collaborators without taking version or transfer policy. */
  constructor(options: {
    graph: CubeReplacementGraph;
    catalog: CubeNodeCatalog;
    history: CubePlacementHistory;
    selection: CubeReplacementSelection;
    logger: Pick<Console, 'error'>;
  }) {
    this.#graph = options.graph;
    this.#catalog = options.catalog;
    this.#history = options.history;
    this.#selection = options.selection;
    this.#logger = options.logger;
  }

  /** Atomically replace one Cube after proving all connected boundaries remain available. */
  replace(request: CubeVersionReplacementRequest): void {
    const connections = captureConnections(this.#graph, request);
    let sourceRemoved = false;
    let targetInserted = false;
    this.#history.beforeChange?.();
    try {
      this.#remove(request.source);
      sourceRemoved = true;
      this.#insert(request.target);
      targetInserted = true;
      reconnectTarget(request.target, connections);
      this.#selection.replace(request.source, request.target);
      this.#history.setDirtyCanvas?.(true, true);
    } catch (error: unknown) {
      if (sourceRemoved) {
        try {
          if (targetInserted) this.#remove(request.target);
          this.#insert(request.source);
          reconnectSource(request.source, connections);
          this.#selection.replace(request.target, request.source);
        } catch (rollbackError: unknown) {
          this.#logger.error('SugarCubes: Cube version rollback failed.', rollbackError);
        }
      }
      throw error;
    } finally {
      this.#history.afterChange?.();
    }
  }

  /** Remove one graph node and make catalog cleanup idempotent with host lifecycle hooks. */
  #remove(node: CubeNode): void {
    this.#graph.remove(node);
    this.#catalog.remove(readInstanceId(node));
  }

  /** Insert one graph node and make catalog registration idempotent with host lifecycle hooks. */
  #insert(node: CubeNode): void {
    this.#graph.add(node);
    this.#catalog.add(node);
  }
}

interface InputConnection {
  outside: NativeCubeConnection['source'];
  outsideSlot: number;
  sourceSlot: number;
  targetSlot: number;
}

interface OutputConnection {
  outside: NativeCubeConnection['source'];
  outsideSlot: number;
  sourceSlot: number;
  targetSlot: number;
}

interface CapturedConnections {
  inputs: InputConnection[];
  outputs: OutputConnection[];
}

/** Capture and validate every live connection before graph mutation begins. */
function captureConnections(
  graph: CubeReplacementGraph,
  request: CubeVersionReplacementRequest,
): CapturedConnections {
  const inputs: InputConnection[] = [];
  request.source.inputs.forEach((input, sourceSlot) => {
    if (input.link == null) return;
    const boundaryId = request.sourceBoundaries.inputIdBySlot.get(sourceSlot);
    const targetSlot = boundaryId
      ? request.targetBoundaries.inputSlotById.get(boundaryId)
      : undefined;
    if (targetSlot == null) throw incompatibleBoundary('input', input.name, boundaryId);
    const connection = readNativeCubeConnection(graph, input.link);
    const outside = connection.source;
    inputs.push({
      outside,
      outsideSlot: connection.sourceSlot,
      sourceSlot,
      targetSlot,
    });
  });
  const outputs: OutputConnection[] = [];
  request.source.outputs.forEach((output, sourceSlot) => {
    for (const linkId of output.links ?? []) {
      const boundaryId = request.sourceBoundaries.outputIdBySlot.get(sourceSlot);
      const targetSlot = boundaryId
        ? request.targetBoundaries.outputSlotById.get(boundaryId)
        : undefined;
      if (targetSlot == null) throw incompatibleBoundary('output', output.name, boundaryId);
      const connection = readNativeCubeConnection(graph, linkId);
      const outside = connection.target;
      outputs.push({
        outside,
        outsideSlot: connection.targetSlot,
        sourceSlot,
        targetSlot,
      });
    }
  });
  return { inputs, outputs };
}

/** Reconnect compatible boundaries to the replacement node. */
function reconnectTarget(target: CubeNode, connections: CapturedConnections): void {
  for (const connection of connections.inputs) {
    connection.outside.connect(connection.outsideSlot, target, connection.targetSlot);
  }
  for (const connection of connections.outputs) {
    target.connect(connection.targetSlot, connection.outside, connection.outsideSlot);
  }
}

/** Restore original slot indices when a replacement mutation rolls back. */
function reconnectSource(source: CubeNode, connections: CapturedConnections): void {
  for (const connection of connections.inputs) {
    connection.outside.connect(connection.outsideSlot, source, connection.sourceSlot);
  }
  for (const connection of connections.outputs) {
    source.connect(connection.sourceSlot, connection.outside, connection.outsideSlot);
  }
}

/** Explain a fail-closed version switch without dropping a user's connection. */
function incompatibleBoundary(kind: string, name: unknown, id: string | undefined): Error {
  const label = typeof name === 'string' && name.trim() ? name.trim() : id || 'unknown';
  return new Error(`Cube version cannot preserve connected ${kind} '${label}'.`);
}
