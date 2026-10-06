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

/** Describe accepted implicit edges alongside native graph connections. */
export interface WorkflowNodeConnection {
  readonly source: GraphId;
  readonly target: GraphId;
}

/** Resolve dependency order with spatial reading order for otherwise independent nodes. */
export function orderWorkflowNodes(
  graph: ComfyGraph,
  connections: readonly WorkflowNodeConnection[] = [],
): ComfyNode[] {
  const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort(
    (a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0),
  );
  const byId = new Map(nodes.map((node) => [String(node.id), node]));
  return orderNodesUpstreamFirst(nodes, graph, connections).flatMap((id) => {
    const node = byId.get(id);
    return node ? [node] : [];
  });
}
