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
/** Verify invalid nested wrappers remain recoverable but separate from root Cubes. */

import { describe, expect, test } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import {
  CubeGraphInventory,
  NestedCubeWorkflowError,
} from '../../frontend/comfyui/ui/cube/node/CubeGraphInventory.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

describe('CubeGraphInventory', () => {
  test('classifies root and nested SugarCubes without mistaking ordinary Subgraphs', () => {
    const rootCube = cubeNode('root');
    const nestedCube = cubeNode('nested');
    const ordinarySubgraph = {
      id: 'ordinary-node',
      properties: {},
      subgraph: { id: 'ordinary-definition', extra: {} },
      isSubgraphNode: () => true,
    };
    const graph = {
      _nodes: [rootCube, ordinarySubgraph],
      subgraphs: new Map([
        [
          'cube-implementation',
          { id: 'cube-implementation', name: 'Cube implementation', _nodes: [ordinarySubgraph] },
        ],
        [
          'ordinary-implementation',
          { id: 'ordinary-implementation', name: 'Ordinary Subgraph', _nodes: [nestedCube] },
        ],
      ]),
    };

    const snapshot = new CubeGraphInventory(graph).snapshot();

    expect(snapshot.rootCubes).toEqual([rootCube]);
    expect(snapshot.nestedCubes).toEqual([
      expect.objectContaining({
        definitionId: 'ordinary-implementation',
        definitionName: 'Ordinary Subgraph',
        node: nestedCube,
      }),
    ]);
  });

  test('blocks persistence and execution without altering historical graph data', () => {
    const nestedCube = cubeNode('nested');
    const definition = { id: 'definition', name: 'Invalid history', _nodes: [nestedCube] };
    const graph = { _nodes: [], subgraphs: new Map([['definition', definition]]) };
    const inventory = new CubeGraphInventory(graph);

    expect(() => inventory.assertNoNestedCubes('save SugarCubes')).toThrow(NestedCubeWorkflowError);
    expect(definition._nodes).toEqual([nestedCube]);
    expect(graph.subgraphs.get('definition')).toBe(definition);
  });
});

/** Build a real-subgraph-node double with durable SugarCube identity. */
function cubeNode(instanceId: string): CubeNode {
  const subgraph = {
    id: `definition-${instanceId}`,
    name: instanceId,
    _nodes: [],
    inputs: [],
    outputs: [],
  } as unknown as NativeCubeSubgraph;
  return {
    id: instanceId,
    type: subgraph.id,
    title: instanceId,
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: instanceId },
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
