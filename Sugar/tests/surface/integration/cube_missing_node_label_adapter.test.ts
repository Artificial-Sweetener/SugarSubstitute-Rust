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
/** Verify Cube-aware missing-node hints before Comfy warning storage. */

import { CubeMissingNodeLabelAdapter } from '../../../frontend/comfyui/ui/affordance/CubeMissingNodeLabelAdapter.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('adapts Cube execution paths and preserves ordinary Subgraph hints', () => {
  const catalog = new CubeNodeCatalog();
  catalog.add(cubeNode());
  const cubeMissing = { nodeId: 'cube-1:8', hint: "in subgraph 'Old'" };
  const ordinaryMissing = { nodeId: 'ordinary:9', hint: "in subgraph 'Nested'" };
  const adapter = new CubeMissingNodeLabelAdapter(catalog);
  expect(adapter.adapt([cubeMissing, ordinaryMissing])).toBe(1);
  expect(cubeMissing.hint).toBe("in Cube '<Cube>'");
  expect(ordinaryMissing.hint).toBe("in subgraph 'Nested'");
});

/** Build a Cube with markup-like metadata to verify literal hint construction. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1', default_alias: '<Cube>' },
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
