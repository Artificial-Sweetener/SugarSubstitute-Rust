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
/** Prove the Cube index follows the graph: a Cube the graph dropped never blocks its successor. */
import { expect, test } from '@jest/globals';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

/** One Cube node with the identity a saved workflow gave it. */
function cube(graph: ComfyGraph): CubeNode {
  const node = {
    id: 1,
    graph,
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: 'saved-identity' } },
  } as unknown as CubeNode;
  graph._nodes?.push(node);
  return node;
}

test('a Cube with an identity the graph still holds is refused', () => {
  const graph: ComfyGraph = { _nodes: [] };
  const catalog = new CubeNodeCatalog();
  catalog.add(cube(graph));
  expect(() => catalog.add(cube(graph))).toThrow("Cube node 'saved-identity' already exists.");
});

test('a Cube takes the place of one with its identity that the graph has dropped', () => {
  const graph: ComfyGraph = { _nodes: [] };
  const catalog = new CubeNodeCatalog();
  catalog.add(cube(graph));
  // Another workflow is loaded into the same graph: its nodes replace the ones indexed.
  graph._nodes = [];
  const successor = cube(graph);
  catalog.add(successor);
  expect(catalog.get('saved-identity')).toBe(successor);
});
