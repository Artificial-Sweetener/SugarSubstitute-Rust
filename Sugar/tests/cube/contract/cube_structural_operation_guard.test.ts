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
/** Verify action-boundary guards around Comfy structural graph operations. */

import { jest } from '@jest/globals';
import { CubeStructuralOperationGuard } from '../../../frontend/comfyui/ui/affordance/CubeStructuralOperationGuard.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('blocks Cube operands while preserving native ordinary operations and disposal', () => {
  const nativeConvert = jest.fn((_items: unknown) => ({ node: {} }));
  const nativeUnpack = jest.fn((_node: unknown) => 'unpacked');
  const rootGraph = { _nodes: [], convertToSubgraph: nativeConvert, unpackSubgraph: nativeUnpack };
  const catalog = new CubeNodeCatalog();
  const cube = cubeNode();
  catalog.add(cube);
  const feedback = { push: jest.fn() };
  const guard = new CubeStructuralOperationGuard({ rootGraph, nodes: catalog, feedback });
  const ordinary = { isSubgraphNode: () => true, subgraph: {} };

  expect(rootGraph.convertToSubgraph(new Set([cube]))).toBeNull();
  expect(rootGraph.unpackSubgraph(cube)).toBeNull();
  expect(nativeConvert).not.toHaveBeenCalled();
  expect(nativeUnpack).not.toHaveBeenCalled();
  expect(rootGraph.convertToSubgraph(new Set([ordinary]))).toEqual({ node: {} });
  expect(rootGraph.unpackSubgraph(ordinary)).toBe('unpacked');
  guard.dispose();
  expect(rootGraph.unpackSubgraph(cube)).toBe('unpacked');
});

/** Build one complete marked Cube operand. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
