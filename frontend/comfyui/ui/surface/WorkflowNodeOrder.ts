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
import { readNativeCubePosition } from '../cube/graph/NativeCubePositions.js';
import { cubePlacementOrder } from '../workflow/domain/CubeGraphOrder.js';

/** Describe accepted implicit edges alongside native graph connections. */
export interface WorkflowNodeConnection {
  readonly source: GraphId;
  readonly target: GraphId;
}

/** Keep loose-node dependencies while reading Cube slots in their canonical native position order. */
export function orderWorkflowNodes(
  graph: ComfyGraph,
  connections: readonly WorkflowNodeConnection[] = [],
): ComfyNode[] {
  const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort(
    (a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0),
  );
  const byId = new Map(nodes.map((node) => [String(node.id), node]));
  const ordered = orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
    const node = byId.get(id);
    return node ? [node] : [];
  });
  const cubes = nodes.filter(isCubeNode);
  const byInstance = new Map(
    cubes.map((cube) => [String(requireCubeIdentity(cube).instance_id ?? cube.id), cube]),
  );
  const cubeOrder = cubePlacementOrder(
    cubes.map((cube) => ({
      instanceId: String(requireCubeIdentity(cube).instance_id ?? cube.id),
      nodeId: String(cube.id),
      position: readNativeCubePosition(cube.pos),
    })),
  );
  let cubeIndex = 0;
  return ordered.map((node) => {
    if (!isCubeNode(node)) return node;
    const id = cubeOrder[cubeIndex++];
    return (id === undefined ? undefined : byInstance.get(id)) ?? node;
  });
}
