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
/** Guard native structural mutations at graph-instance boundaries. */

import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import { isRecord } from '../types/common.js';

interface GuardFeedback {
  push?(severity: string, summary: string, detail?: string): unknown;
}

interface GuardedGraph {
  convertToSubgraph?: (...args: unknown[]) => unknown;
  unpackSubgraph?: (...args: unknown[]) => unknown;
  _nodes?: unknown[];
}

interface GraphMethods {
  convertToSubgraph?: (...args: unknown[]) => unknown;
  unpackSubgraph?: (...args: unknown[]) => unknown;
}

/** Prevent Comfy convert/unpack implementations from ever receiving a Cube operand. */
export class CubeStructuralOperationGuard {
  readonly #rootGraph: object;
  readonly #nodes: CubeNodeCatalog;
  readonly #feedback: GuardFeedback | null;
  readonly #originals = new Map<GuardedGraph, GraphMethods>();
  readonly #unsubscribe: () => void;

  /** Bind only live graph instances owned by the current Comfy workflow. */
  constructor(options: {
    rootGraph: object;
    nodes: CubeNodeCatalog;
    feedback?: GuardFeedback | null;
  }) {
    this.#rootGraph = options.rootGraph;
    this.#nodes = options.nodes;
    this.#feedback = options.feedback ?? null;
    this.#unsubscribe = options.nodes.subscribe(() => this.refresh());
    this.refresh();
  }

  /** Guard the root graph plus every currently reachable Cube definition graph. */
  refresh(): void {
    this.#guardGraphTree(this.#rootGraph);
    for (const node of this.#nodes.list()) this.#guardGraphTree(node.subgraph);
  }

  /** Restore host methods when the graph-bound Cube runtime is replaced. */
  dispose(): void {
    this.#unsubscribe();
    for (const [graph, methods] of this.#originals) {
      if (methods.convertToSubgraph) graph.convertToSubgraph = methods.convertToSubgraph;
      if (methods.unpackSubgraph) graph.unpackSubgraph = methods.unpackSubgraph;
    }
    this.#originals.clear();
  }

  /** Patch one graph exactly once and recursively discover nested native definitions. */
  #guardGraphTree(value: object, visited = new Set<object>()): void {
    if (visited.has(value) || !isRecord(value)) return;
    visited.add(value);
    const graph = value as GuardedGraph;
    if (!this.#originals.has(graph)) this.#guardGraph(graph);
    if (!Array.isArray(graph._nodes)) return;
    for (const node of graph._nodes) {
      if (!isRecord(node) || !isRecord(node.subgraph)) continue;
      this.#guardGraphTree(node.subgraph, visited);
    }
  }

  /** Wrap semantic graph operations while preserving ordinary Subgraph behavior verbatim. */
  #guardGraph(graph: GuardedGraph): void {
    const methods: GraphMethods = {};
    if (typeof graph.convertToSubgraph === 'function') {
      const nativeConvert = graph.convertToSubgraph;
      methods.convertToSubgraph = nativeConvert;
      graph.convertToSubgraph = (...args: unknown[]) => {
        if (containsCube(args[0])) return this.#blocked('convert');
        return nativeConvert.apply(graph, args);
      };
    }
    if (typeof graph.unpackSubgraph === 'function') {
      const nativeUnpack = graph.unpackSubgraph;
      methods.unpackSubgraph = nativeUnpack;
      graph.unpackSubgraph = (...args: unknown[]) => {
        if (isCubeNode(args[0])) return this.#blocked('unpack');
        return nativeUnpack.apply(graph, args);
      };
    }
    this.#originals.set(graph, methods);
  }

  /** Give stale command paths actionable feedback without exposing a destructive fallback. */
  #blocked(operation: 'convert' | 'unpack'): null {
    this.#feedback?.push?.(
      'info',
      'SugarCube action unavailable',
      operation === 'convert'
        ? 'A SugarCube is already a reusable graph unit and cannot be converted to a Subgraph.'
        : 'A SugarCube cannot be unpacked. Open it to edit its implementation.',
    );
    return null;
  }
}

/** Read any iterable selection without assuming a Set implementation. */
function containsCube(value: unknown): boolean {
  if (!value || typeof value !== 'object') return isCubeNode(value);
  const iterator = Reflect.get(value, Symbol.iterator);
  if (typeof iterator !== 'function') return isCubeNode(value);
  return Array.from(value as Iterable<unknown>).some(isCubeNode);
}
