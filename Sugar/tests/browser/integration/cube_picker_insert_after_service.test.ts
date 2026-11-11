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
/** Verify picker selection inserts atomically without creating root graph links. */

import { expect, jest, test } from '@jest/globals';
import type { ComfyCubeRuntime } from '../../../frontend/comfyui/ui/cube/ComfyCubeRuntime.js';
import type { PlacedCube } from '../../../frontend/comfyui/ui/cube/CubePlacementService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubePickerPlacementAdapter } from '../../../frontend/comfyui/ui/picker/CubePickerPlacementAdapter.js';
import { CubePickerInsertAfterService } from '../../../frontend/comfyui/ui/picker/CubePickerInsertAfterService.js';

test('places beside the source, displaces downstream Cubes, and leaves hard links untouched', () => {
  const source = cube('source', [20, 40], 220);
  const downstream = cube('downstream', [300, 80], 360);
  const inserted = cube('inserted', [0, 0], 340);
  const nodes = new CubeNodeCatalog();
  nodes.add(source);
  nodes.add(downstream);
  const placed: PlacedCube = {
    node: inserted,
    subgraph: inserted.subgraph,
    warnings: [],
    internalNodeCount: 2,
  };
  const placeBatch = jest.fn(
    (_items: unknown, finalize: (items: readonly PlacedCube[]) => void): readonly PlacedCube[] => {
      nodes.add(inserted);
      finalize([placed]);
      return [placed];
    },
  );
  const runtime = {
    nodes,
    registerSubgraphs: jest.fn(() => ({ warnings: [], createdIds: ['definition-inserted'] })),
    discardSubgraphs: jest.fn(),
    placement: { placeBatch },
  } as unknown as ComfyCubeRuntime;
  const prepare = jest.fn(() => ({
    descriptor: {
      cubeId: 'local/inserted.cube',
      displayName: 'Inserted',
    },
    payload: { cube: { cube_id: 'local/inserted.cube' } },
    options: { instanceAlias: 'Inserted', position: [300, 40] },
  }));
  const placement = {
    prepare,
    reportWarnings: jest.fn(),
  } as unknown as CubePickerPlacementAdapter;

  const result = new CubePickerInsertAfterService({
    placement,
    getRuntime: () => runtime,
  }).insert('source', 'SugarCubes.Cube.inserted');

  expect(result).toBe(inserted);
  expect(prepare).toHaveBeenCalledWith('SugarCubes.Cube.inserted', [300, 40]);
  expect([...inserted.pos]).toEqual([300, 40]);
  expect([...downstream.pos]).toEqual([700, 80]);
  expect(source.outputs[0]?.links).toBeNull();
  for (const node of [source, inserted, downstream]) expect(node.connect).not.toHaveBeenCalled();
  expect(runtime.discardSubgraphs).not.toHaveBeenCalled();
});

test('discards registered definitions when graph insertion fails', () => {
  const source = cube('source', [20, 40], 220);
  const nodes = new CubeNodeCatalog();
  nodes.add(source);
  const runtime = {
    nodes,
    registerSubgraphs: () => ({ warnings: [], createdIds: ['created-definition'] }),
    discardSubgraphs: jest.fn(),
    placement: {
      placeBatch: () => {
        throw new Error('graph add failed');
      },
    },
  } as unknown as ComfyCubeRuntime;
  const placement = {
    prepare: () => ({
      descriptor: { cubeId: 'local/inserted.cube' },
      payload: { cube: { cube_id: 'local/inserted.cube' } },
      options: {},
    }),
    reportWarnings: jest.fn(),
  } as unknown as CubePickerPlacementAdapter;

  expect(() =>
    new CubePickerInsertAfterService({ placement, getRuntime: () => runtime }).insert(
      'source',
      'SugarCubes.Cube.inserted',
    ),
  ).toThrow('graph add failed');
  expect(runtime.discardSubgraphs).toHaveBeenCalledWith(['created-definition']);
});

/** Build the native Cube surface required by insertion geometry. */
function cube(instanceId: string, position: [number, number], width: number): CubeNode {
  return {
    id: instanceId,
    type: instanceId,
    title: instanceId,
    pos: [...position],
    size: [width, 160],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: instanceId },
      sugarcubes_surface: {},
    },
    inputs: [{ name: 'image', type: 'IMAGE' }],
    outputs: [{ name: 'image', type: 'IMAGE', links: null }],
    subgraph: {
      id: `definition-${instanceId}`,
      name: instanceId,
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
    },
    isSubgraphNode: () => true,
    connect: jest.fn(),
    serialize: () => ({}),
  };
}
