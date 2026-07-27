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
/** Verify Cube-facing names remain distinct from canonical Sugar DSL bindings. */

import { expect, test } from '@jest/globals';
import {
  synchronizeCubeOutputSurfaceNames,
  CubeOutputSurfaceSynchronizer,
} from '../../frontend/comfyui/ui/cube/CubeOutputSurfaceSynchronizer.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('uses concise type names for generic native Cube outputs', () => {
  const node = cubeNode([
    { name: 'IMAGE', type: 'IMAGE' },
    { name: 'IMAGE', type: 'IMAGE' },
  ]);

  expect(synchronizeCubeOutputSurfaceNames(node)).toBe(true);
  expect(node.subgraph.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
  expect(node.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
});

test('removes the canonical output prefix only from the Cube surface', () => {
  const node = cubeNode([
    { name: 'output.image', type: 'IMAGE' },
    { name: 'output.image2', type: 'IMAGE' },
  ]);

  synchronizeCubeOutputSurfaceNames(node);

  expect(node.subgraph.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
  expect(node.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
});

test('synchronizes outputs introduced after a Cube is registered', () => {
  const node = cubeNode([{ name: 'IMAGE', type: 'IMAGE' }]);
  const catalog = new CubeNodeCatalog();
  const events = new EventTarget();
  const synchronizer = new CubeOutputSurfaceSynchronizer(catalog, events);

  catalog.add(node);
  node.subgraph.outputs.push({ name: 'IMAGE', type: 'IMAGE' });
  node.outputs.push({ name: 'IMAGE', type: 'IMAGE' });
  events.dispatchEvent(graphChangeEvent());

  expect(node.subgraph.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
  expect(node.outputs.map((output) => output.name)).toEqual(['image', 'image2']);
  synchronizer.dispose();
});

/** Build a managed Cube node with matching native interface output slots. */
function cubeNode(outputs: Array<{ name: string; type: string }>): CubeNode {
  const subgraph = {
    id: 'definition',
    name: 'Cube',
    _nodes: [],
    inputs: [],
    outputs: outputs.map((output) => ({ ...output })),
  } as unknown as NativeCubeSubgraph;
  return {
    id: 'instance',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'instance' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: outputs.map((output) => ({ ...output })),
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

/** Build the public canvas event Comfy emits after completing one graph mutation. */
function graphChangeEvent(): CustomEvent<{ subType: string }> {
  return new CustomEvent('litegraph:canvas', { detail: { subType: 'after-change' } });
}
