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
/** Resolve Cube editor and selection context from Comfy-owned graph objects. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { isCubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { CubeNodeCatalog } from '../cube/node/CubeNodeCatalog.js';
import { isRecord } from '../types/common.js';

/** Describe the active graph's position within one Cube definition hierarchy. */
export interface CubeEditorContext {
  node: CubeNode;
  path: readonly object[];
  isCubeRoot: boolean;
}

/** Classify selected operands once for every Cube affordance consumer. */
export interface CubeSelectionContext {
  items: readonly unknown[];
  cubeNodes: readonly CubeNode[];
  ordinarySubgraphNodes: readonly object[];
  containsCube: boolean;
  isSingleCube: boolean;
  isSingleOrdinarySubgraph: boolean;
}

interface NativeSubgraphNode {
  subgraph: object;
}

/** Own graph traversal and operand classification for Cube-aware host integration. */
export class CubeEditorContextResolver {
  readonly #nodes: CubeNodeCatalog;

  /** Bind the live root-Cube catalog without taking graph lifecycle ownership. */
  constructor(nodes: CubeNodeCatalog) {
    this.#nodes = nodes;
  }

  /** Resolve the active graph only when it belongs to a known root Cube. */
  resolveEditor(currentGraph: object | null): CubeEditorContext | null {
    if (!currentGraph) return null;
    for (const node of this.#nodes.list()) {
      const path = findGraphPath(node.subgraph, currentGraph);
      if (path) return { node, path, isCubeRoot: path.length === 1 };
    }
    return null;
  }

  /** Classify a host selection without relying on renderer-specific constructors. */
  resolveSelection(items: Iterable<unknown>): CubeSelectionContext {
    const selected = [...items];
    const cubeNodes = selected.filter(isCubeNode);
    const ordinarySubgraphNodes = selected.filter(
      (item): item is object => !isCubeNode(item) && isNativeSubgraphNode(item),
    );
    return {
      items: selected,
      cubeNodes,
      ordinarySubgraphNodes,
      containsCube: cubeNodes.length > 0,
      isSingleCube: selected.length === 1 && cubeNodes.length === 1,
      isSingleOrdinarySubgraph: selected.length === 1 && ordinarySubgraphNodes.length === 1,
    };
  }
}

/** Traverse actual nested SubgraphNodes while retaining live graph object identity. */
function findGraphPath(
  graph: object,
  target: object,
  visited = new Set<object>(),
): object[] | null {
  if (sameGraph(graph, target)) return [graph];
  if (visited.has(graph)) return null;
  visited.add(graph);
  if (!isRecord(graph) || !Array.isArray(graph._nodes)) return null;
  for (const node of graph._nodes) {
    if (!isNativeSubgraphNode(node) || !isRecord(node.subgraph)) continue;
    const nestedPath = findGraphPath(node.subgraph, target, visited);
    if (nestedPath) return [graph, ...nestedPath];
  }
  return null;
}

/** Match restored definitions by identity before using their stable definition id. */
function sameGraph(left: object, right: object): boolean {
  if (left === right) return true;
  const leftId = readGraphId(left);
  return leftId.length > 0 && leftId === readGraphId(right);
}

/** Read a graph definition id at the dynamic host boundary. */
function readGraphId(graph: object): string {
  return isRecord(graph) && typeof graph.id === 'string' ? graph.id.trim() : '';
}

/** Recognize structural SubgraphNodes without depending on a private constructor export. */
function isNativeSubgraphNode(value: unknown): value is NativeSubgraphNode {
  if (!isRecord(value) || typeof value.isSubgraphNode !== 'function') return false;
  return value.isSubgraphNode.call(value) === true && isRecord(value.subgraph);
}
