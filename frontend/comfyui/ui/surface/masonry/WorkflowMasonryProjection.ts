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
/** Project graph membership and reading order without changing workflow state. */
import type { ComfyGraph, ComfyNode, GraphId } from '../../types/graph.js';
import type { CubeNode } from '../../cube/node/ComfyCubeNodeFactory.js';
import { requireCubeIdentity } from '../../cube/node/ComfyCubeNodeFactory.js';
import { orderNodesUpstreamFirst } from '../CubeFaceCardPolicy.js';
import { readCubeFaceCardState, type CubeFaceCardTarget } from '../CubeFaceCardStateService.js';
import { projectNodeCards, type ProjectedNodeCard } from '../NodeCardProjection.js';

export interface WorkflowMasonryCard extends ProjectedNodeCard {
  columnSpan: number;
}

export interface WorkflowMasonrySection {
  id: string;
  label: string;
  cube: CubeNode | null;
  target: CubeFaceCardTarget;
  cards: WorkflowMasonryCard[];
}

export interface WorkflowMasonryConnection {
  source: GraphId;
  target: GraphId;
}

/** Keep real Cubes and consecutive loose nodes in distinct runs of the shared grid. */
export function projectWorkflowMasonry(
  graph: ComfyGraph,
  cubes: readonly CubeNode[],
  connections: readonly WorkflowMasonryConnection[] = [],
): WorkflowMasonrySection[] {
  const nodes = [...(graph._nodes ?? graph.nodes ?? [])].sort(
    (a, b) => (a.pos?.[0] ?? 0) - (b.pos?.[0] ?? 0) || (a.pos?.[1] ?? 0) - (b.pos?.[1] ?? 0),
  );
  const byId = new Map(nodes.map((node) => [String(node.id), node]));
  const cubeSet = new Set<ComfyNode>(cubes);
  const sections: WorkflowMasonrySection[] = [];
  const looseMembers = new Map<string, ComfyNode[]>();
  let loose: WorkflowMasonrySection | null = null;
  for (const id of orderNodesUpstreamFirst(nodes, graph, connections)) {
    const node = byId.get(id);
    if (!node) continue;
    const cube = cubeSet.has(node) ? cubes.find((candidate) => candidate === node) : undefined;
    if (cube) {
      loose = null;
      const identity = requireCubeIdentity(cube);
      sections.push({
        id: `cube:${String(identity.instance_id ?? cube.id)}`,
        label: cube.title?.trim() || 'SugarCube',
        cube,
        target: cube,
        cards: orderedCards(
          cube.subgraph._nodes,
          cube.subgraph,
          `cube:${String(cube.id)}`,
          cube,
          cube,
        ),
      });
      continue;
    }
    if (!loose) {
      loose = { id: `loose:${id}`, label: 'Workflow nodes', cube: null, target: graph, cards: [] };
      sections.push(loose);
      looseMembers.set(loose.id, []);
    }
    looseMembers.get(loose.id)?.push(node);
  }
  for (const section of sections) {
    if (!section.cube)
      section.cards = orderedCards(looseMembers.get(section.id) ?? [], graph, 'root', graph);
  }
  return sections;
}

/** Project the exact Cube-face decisions with namespaced placement identities. */
function orderedCards(
  nodes: readonly ComfyNode[],
  graph: ComfyGraph,
  prefix: string,
  target: CubeFaceCardTarget,
  cube?: CubeNode,
): WorkflowMasonryCard[] {
  return projectNodeCards(nodes, readCubeFaceCardState(target), graph, cube).map((card) => ({
    ...card,
    id: `${prefix}:${card.id}`,
    columnSpan: card.columnSpan ?? 1,
  }));
}
