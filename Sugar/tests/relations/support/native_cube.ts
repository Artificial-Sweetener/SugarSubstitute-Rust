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
/** Supply real native Cube boundaries for relation behavior proofs. */
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

/** Build an identified Cube while keeping its members on the actual subgraph boundary. */
export function nativeCube(id: string, members: NativeGraphNode[], x = 0): CubeNode {
  const cube: CubeNode = {
    id,
    title: id,
    type: 'Subgraph',
    pos: [x, 0],
    size: [720, 480],
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: id } },
    inputs: [],
    outputs: [],
    widgets: [],
    connect() {},
    serialize: () => ({}),
    isSubgraphNode: () => true,
    subgraph: {
      id: `definition-${id}`,
      name: id,
      _nodes: members,
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      configure() {},
      addInput: () => ({ connect() {} }),
      addOutput: () => ({ connect() {} }),
    },
  };
  for (const member of members) member.graph = cube.subgraph;
  return cube;
}
