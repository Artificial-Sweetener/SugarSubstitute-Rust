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
/** Share workflow reading order between masonry and graph-based feature inventories. */
import type { ComfyGraph, ComfyNode, GraphId } from '../types/graph.js';
import { orderNodesUpstreamFirst } from './CubeFaceCardPolicy.js';
import { isCubeNode, requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';

/** Describe accepted implicit edges alongside native graph connections. */
export interface WorkflowNodeConnection {
  readonly source: GraphId;
  readonly target: GraphId;
}

/**
 * Read nodes upstream first and fill the Cube places with the stack order, so masonry, the stack
 * and every inventory list Cubes the way the Cube chain reads them.
 */
export function orderWorkflowNodes(
  graph: ComfyGraph,
  connections: readonly WorkflowNodeConnection[] = [],
  cubeOrder: readonly string[] | null = null,
): ComfyNode[] {
  const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort(
    (a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0),
  );
  const byId = new Map(nodes.map((node) => [String(node.id), node]));
  const ordered = orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
    const node = byId.get(id);
    return node ? [node] : [];
  });
  if (!cubeOrder) return ordered;
  const byInstance = new Map(
    nodes
      .filter(isCubeNode)
      .map((cube) => [String(requireCubeIdentity(cube).instance_id ?? cube.id), cube]),
  );
  const stack = cubeOrder.flatMap((id) => {
    const cube = byInstance.get(id);
    return cube ? [cube] : [];
  });
  if (stack.length !== byInstance.size) return ordered;
  let cubeIndex = 0;
  return ordered.map((node) => (isCubeNode(node) ? (stack[cubeIndex++] ?? node) : node));
}
