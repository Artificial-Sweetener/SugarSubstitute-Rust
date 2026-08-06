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
/** Specify native Cube-node creation and adoption at the Comfy boundary. */

import { jest } from '@jest/globals';
import {
  ComfyCubeNodeFactory,
  type CubeNode,
} from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

describe('ComfyCubeNodeFactory', () => {
  test('creates one detached real subgraph node for an insertion owner', () => {
    const subgraph = nativeSubgraph('cube-definition');
    const node = nativeSubgraphNode(subgraph);
    const createNode = jest.fn(() => node);
    const factory = new ComfyCubeNodeFactory({ createNode });

    const cube = factory.create({
      subgraph,
      instanceId: 'cube-instance',
      title: 'Text to Image',
      position: [40, 60],
      size: [900, 640],
      identity: { cube_id: 'sdxl/text-to-image.cube' },
      surface: { schema: 1, revealed: true },
    });

    expect(createNode).toHaveBeenCalledWith('cube-definition');
    expect(cube).toBe(node);
    expect(cube).toMatchObject({
      id: 'cube-instance',
      title: 'Text to Image',
      pos: [40, 60],
      size: [900, 640],
    });
    expect(cube.properties).toMatchObject({
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { cube_id: 'sdxl/text-to-image.cube' },
      sugarcubes_surface: { schema: 1, revealed: true },
    });
    expect(cube.serialize).toBeDefined();
  });

  test('adopts Comfy conversion output without adding or replacing the node', () => {
    const subgraph = nativeSubgraph('authored-definition');
    const node = nativeSubgraphNode(subgraph);
    node.id = 27;
    const factory = new ComfyCubeNodeFactory({
      createNode: jest.fn(() => null),
    });

    const cube = factory.adopt(node, {
      subgraph,
      instanceId: 'authored-instance',
      title: 'Authored Cube',
      position: [20, 30],
      size: [720, 480],
      identity: { cube_id: 'local/Authored.cube' },
      surface: {},
    });

    expect(cube).toBe(node);
    expect(cube.id).toBe(27);
    expect(cube.properties.sugarcubes_cube).toMatchObject({
      cube_id: 'local/Authored.cube',
      instance_id: 'authored-instance',
    });
  });

  test('rejects a generic node instead of disguising it as a Cube', () => {
    const subgraph = nativeSubgraph('cube-definition');
    const generic = {
      ...nativeSubgraphNode(subgraph),
      isSubgraphNode: () => false,
    };
    const factory = new ComfyCubeNodeFactory({
      createNode: () => generic,
    });

    expect(() =>
      factory.create({
        subgraph,
        instanceId: 'cube-instance',
        title: 'Invalid',
        position: [0, 0],
        size: [720, 480],
        identity: {},
        surface: {},
      }),
    ).toThrow('real Comfy subgraph node');
  });
});

/** Build a minimal registered native subgraph definition. */
function nativeSubgraph(id: string): NativeCubeSubgraph {
  return {
    id,
    name: id,
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    addInput() {
      throw new Error('not used');
    },
    addOutput() {
      throw new Error('not used');
    },
    configure() {},
  } as unknown as NativeCubeSubgraph;
}

/** Build a minimal real-node host double with native lifecycle methods. */
function nativeSubgraphNode(subgraph: NativeCubeSubgraph): CubeNode {
  return {
    id: -1,
    type: subgraph.id,
    title: subgraph.name,
    pos: [0, 0],
    size: [200, 100],
    properties: {},
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({ id: 'serialized' }),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}
