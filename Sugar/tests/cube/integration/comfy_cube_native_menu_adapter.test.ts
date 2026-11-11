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
/** Verify final native menu filtering for Cube operands only. */

import { jest } from '@jest/globals';
import { ComfyCubeNativeMenuAdapter } from '../../../frontend/comfyui/ui/affordance/ComfyCubeNativeMenuAdapter.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('filters structural Cube menu items and preserves ordinary Subgraphs', () => {
  const nativeNodeMenu = jest.fn((_node: unknown) => [
    { content: 'Convert to Subgraph' },
    { content: 'Edit Subgraph Widgets', callback: jest.fn() },
    { content: 'Unpack Subgraph' },
    { content: 'Delete' },
  ]);
  const canvas = {
    selectedItems: new Set<unknown>(),
    getNodeMenuOptions: nativeNodeMenu,
    getCanvasMenuOptions: () => [{ content: 'Convert to Subgraph' }, { content: 'Add Node' }],
  };
  const adapter = new ComfyCubeNativeMenuAdapter(canvas);
  adapter.install();
  const cube = cubeNode();
  const ordinary = { isSubgraphNode: () => true, subgraph: {} };

  expect(canvas.getNodeMenuOptions(cube)).toEqual([{ content: 'Delete' }]);
  expect(canvas.getNodeMenuOptions(ordinary)).toHaveLength(4);
  canvas.selectedItems.add(cube);
  expect(canvas.getCanvasMenuOptions()).toEqual([{ content: 'Add Node' }]);
  adapter.dispose();
  expect(canvas.getNodeMenuOptions).toBe(nativeNodeMenu);
});

/** Build one complete marked Cube node. */
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
