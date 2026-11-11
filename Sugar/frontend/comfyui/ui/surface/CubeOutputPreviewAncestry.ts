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

/** Resolve upstream nodes that may publish a live preview for one Cube output. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { buildLinkIndex } from '../graph/GraphQuery.js';
import { isRecord } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';

interface PreviewGraph {
  _nodes: readonly ComfyNode[];
}

/** Return upstream node locators in nearest-first graph-distance layers. */
export function upstreamPreviewLocatorLayers(
  cube: CubeNode,
  outputNode: ComfyNode,
  locatorByNode: ReadonlyMap<ComfyNode, string>,
): readonly (readonly string[])[] {
  const graph = findContainingGraph(cube.subgraph, outputNode);
  if (!graph) return [];
  const nodesById = new Map(graph._nodes.map((node) => [String(node.id), node]));
  const links = buildLinkIndex(graph);
  const visited = new Set<string>([String(outputNode.id)]);
  let frontier = [String(outputNode.id)];
  const layers: string[][] = [];

  while (frontier.length > 0) {
    const next: string[] = [];
    const locators: string[] = [];
    for (const nodeId of frontier) {
      for (const link of links.incoming.get(nodeId) ?? []) {
        const upstreamId = readNodeId(link.origin_id ?? link.origin);
        if (!upstreamId || visited.has(upstreamId)) continue;
        visited.add(upstreamId);
        next.push(upstreamId);
        const upstreamNode = nodesById.get(upstreamId);
        const locator = upstreamNode ? locatorByNode.get(upstreamNode) : undefined;
        if (locator) locators.push(locator);
      }
    }
    if (locators.length > 0) layers.push(locators);
    frontier = next;
  }
  return layers;
}

/** Find the nested graph that directly owns one output-boundary source node. */
function findContainingGraph(graph: PreviewGraph, target: ComfyNode): PreviewGraph | null {
  if (graph._nodes.includes(target)) return graph;
  for (const node of graph._nodes) {
    const subgraph = isRecord(node.subgraph) ? node.subgraph : null;
    if (!subgraph || !Array.isArray(subgraph._nodes)) continue;
    const found = findContainingGraph(subgraph as unknown as PreviewGraph, target);
    if (found) return found;
  }
  return null;
}

/** Read one graph node identity accepted by LiteGraph. */
function readNodeId(value: unknown): string | null {
  if (typeof value === 'string') return value.trim() || null;
  return typeof value === 'number' && Number.isFinite(value) ? String(value) : null;
}
