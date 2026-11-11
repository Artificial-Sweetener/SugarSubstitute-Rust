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
/** Prove copied native definitions are isolated atomically and reconciliation is idempotent. */
import { expect, jest, test } from '@jest/globals';
import { NativeCubeDefinitionOwnership } from '../../../frontend/comfyui/ui/cube/graph/NativeCubeDefinitionOwnership.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Provide the host definition capabilities used by isolation without a renderer. */
function definition(id: string): NativeCubeSubgraph {
  return {
    id,
    name: 'Cube',
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add() {},
    remove() {},
    addInput: () => ({ connect() {} }),
    addOutput: () => ({ connect() {} }),
    configure() {},
    asSerialisable: () => ({ id, name: 'Cube', nodes: [] }),
  };
}

/** Retain native root identity and metadata while supplying mutable definition state. */
function cube(id: string, subgraph: NativeCubeSubgraph): CubeNode {
  return {
    id,
    type: subgraph.id,
    title: 'Cube',
    pos: [1, 2],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: id },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

test('retains original roots and metadata and does not clone again after repeated reorder', () => {
  const original = definition('original');
  const first = cube('first', original);
  const second = cube('second', original);
  const registry = new Map([[original.id, original]]);
  const createSubgraph = jest.fn((data: UnknownRecord) => {
    const created = definition(String(data.id));
    registry.set(created.id, created);
    return created;
  });
  const owner = new NativeCubeDefinitionOwnership(
    { subgraphs: registry, createSubgraph },
    () => 'isolated',
    (id) => {
      registry.delete(id);
    },
  );
  owner.reconcile([first, second]);
  const isolated = second.subgraph;
  owner.reconcile([second, first]);
  owner.reconcile([first, second]);
  expect(createSubgraph).toHaveBeenCalledTimes(1);
  expect(first.subgraph).toBe(original);
  expect(second.subgraph).toBe(isolated);
  expect(second).toMatchObject({ id: 'second', type: 'isolated', pos: [1, 2] });
  expect(second.properties.sugarcubes_cube).toEqual({ instance_id: 'second' });
});

test('removes a registered partial definition when native configuration fails', () => {
  const original = definition('original');
  const first = cube('first', original);
  const second = cube('second', original);
  const surface = second.properties.sugarcubes_surface;
  const registry = new Map([[original.id, original]]);
  const owner = new NativeCubeDefinitionOwnership(
    {
      subgraphs: registry,
      createSubgraph(data: UnknownRecord) {
        const created = definition(String(data.id));
        created.configure = () => {
          throw new Error('configuration failed');
        };
        registry.set(created.id, created);
        return created;
      },
    },
    () => 'isolated',
    (id) => {
      registry.delete(id);
    },
  );
  expect(() => owner.reconcile([first, second])).toThrow('configuration failed');
  expect(registry.size).toBe(1);
  expect(second.subgraph).toBe(original);
  expect(second.type).toBe(original.id);
  expect(second.properties.sugarcubes_surface).toBe(surface);
});

test('restores earlier assignments when a later copied definition fails', () => {
  const original = definition('original');
  const first = cube('first', original);
  const second = cube('second', original);
  const third = cube('third', original);
  const surface = second.properties.sugarcubes_surface;
  const registry = new Map([[original.id, original]]);
  let next = 0;
  const owner = new NativeCubeDefinitionOwnership(
    {
      subgraphs: registry,
      createSubgraph(data: UnknownRecord) {
        const created = definition(String(data.id));
        if (registry.size === 2)
          created.configure = () => {
            throw new Error('third failed');
          };
        registry.set(created.id, created);
        return created;
      },
    },
    () => `isolated-${++next}`,
    (id) => {
      registry.delete(id);
    },
  );
  expect(() => owner.reconcile([first, second, third])).toThrow('third failed');
  expect(registry.size).toBe(1);
  expect(second.subgraph).toBe(original);
  expect(third.subgraph).toBe(original);
  expect(second.type).toBe(original.id);
  expect(second.properties.sugarcubes_surface).toBe(surface);
});
