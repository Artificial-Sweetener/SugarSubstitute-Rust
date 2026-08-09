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
import type { ComfyLink, GraphId } from '../../types/graph.js';
import type { CubeVersionBoundaryMap } from './CubeVersionBoundaryMap.js';

interface ConnectableCubeNode extends CubeNode {
  connect(outputSlot: number, targetNode: ConnectableCubeNode, inputSlot: number): unknown;
}

export interface CubeReplacementGraph {
  add(node: CubeNode): void;
  remove(node: CubeNode): void;
  getNodeById(id: GraphId): unknown;
  getLink(id: GraphId): ComfyLink | null;
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
  outside: ConnectableCubeNode;
  outsideSlot: number;
  sourceSlot: number;
  targetSlot: number;
}

interface OutputConnection {
  outside: ConnectableCubeNode;
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
    const link = requireLink(graph, input.link);
    const outside = requireConnectableNode(graph.getNodeById(readOriginId(link)));
    inputs.push({
      outside,
      outsideSlot: requireSlot(link.origin_slot, 'origin'),
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
      const link = requireLink(graph, linkId);
      const outside = requireConnectableNode(graph.getNodeById(readTargetId(link)));
      outputs.push({
        outside,
        outsideSlot: requireSlot(link.target_slot, 'target'),
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
    connection.outside.connect(
      connection.outsideSlot,
      target as ConnectableCubeNode,
      connection.targetSlot,
    );
  }
  for (const connection of connections.outputs) {
    target.connect(connection.targetSlot, connection.outside, connection.outsideSlot);
  }
}

/** Restore original slot indices when a replacement mutation rolls back. */
function reconnectSource(source: CubeNode, connections: CapturedConnections): void {
  for (const connection of connections.inputs) {
    connection.outside.connect(
      connection.outsideSlot,
      source as ConnectableCubeNode,
      connection.sourceSlot,
    );
  }
  for (const connection of connections.outputs) {
    source.connect(connection.sourceSlot, connection.outside, connection.outsideSlot);
  }
}

/** Require one still-live graph link. */
function requireLink(graph: CubeReplacementGraph, id: GraphId): ComfyLink {
  const link = graph.getLink(id);
  if (!link) throw new Error(`Cube connection '${String(id)}' is unavailable.`);
  return link;
}

/** Require a node that can accept a restored LiteGraph connection. */
function requireConnectableNode(value: unknown): ConnectableCubeNode {
  if (!value || typeof value !== 'object' || typeof Reflect.get(value, 'connect') !== 'function') {
    throw new Error('Cube connection endpoint is unavailable.');
  }
  return value as ConnectableCubeNode;
}

/** Read the origin node ID across LiteGraph link representations. */
function readOriginId(link: ComfyLink): GraphId {
  const id = link.origin_id ?? link.origin;
  if (id == null) throw new Error('Cube input connection has no origin node.');
  return id;
}

/** Read the target node ID across LiteGraph link representations. */
function readTargetId(link: ComfyLink): GraphId {
  const id = link.target_id ?? link.target;
  if (id == null) throw new Error('Cube output connection has no target node.');
  return id;
}

/** Require one finite non-negative LiteGraph slot. */
function requireSlot(value: unknown, role: string): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0) {
    throw new Error(`Cube connection has an invalid ${role} slot.`);
  }
  return value;
}

/** Explain a fail-closed version switch without dropping a user's connection. */
function incompatibleBoundary(kind: string, name: unknown, id: string | undefined): Error {
  const label = typeof name === 'string' && name.trim() ? name.trim() : id || 'unknown';
  return new Error(`Cube version cannot preserve connected ${kind} '${label}'.`);
}
