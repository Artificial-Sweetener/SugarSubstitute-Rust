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
/** Verify insert-after Cube geometry preserves order without connecting nodes. */

import { expect, jest, test } from '@jest/globals';
import { CubeNodeInsertAfterCoordinator } from '../../../frontend/comfyui/ui/cube/node/CubeNodeInsertAfterCoordinator.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('inserts repeatedly after the source and shifts every downstream Cube', () => {
  const source = cube('source', [20, 40], 220);
  const firstUpscale = cube('upscale-1', [300, 80], 360);
  const tail = cube('tail', [720, 120], 240);
  const catalog = new CubeNodeCatalog();
  for (const node of [source, firstUpscale, tail]) catalog.add(node);
  const coordinator = new CubeNodeInsertAfterCoordinator(catalog);

  const secondUpscale = cube('upscale-2', [0, 0], 340);
  expect(coordinator.insertionOrigin(source)).toEqual([300, 40]);
  catalog.add(secondUpscale);
  coordinator.insertAfter(source, secondUpscale);

  expect([...secondUpscale.pos]).toEqual([300, 40]);
  expect([...firstUpscale.pos]).toEqual([700, 80]);
  expect([...tail.pos]).toEqual([1120, 120]);

  const thirdUpscale = cube('upscale-3', [0, 0], 280);
  catalog.add(thirdUpscale);
  coordinator.insertAfter(source, thirdUpscale);

  expect([...thirdUpscale.pos]).toEqual([300, 40]);
  expect([...secondUpscale.pos]).toEqual([640, 40]);
  expect([...firstUpscale.pos]).toEqual([1040, 80]);
  expect([...tail.pos]).toEqual([1460, 120]);
  for (const node of [source, firstUpscale, secondUpscale, thirdUpscale, tail]) {
    expect(node.connect).not.toHaveBeenCalled();
  }
});

test('enforces a safe minimum gap when existing Cubes overlap', () => {
  const source = cube('source', [20, 40], 220);
  const neighbor = cube('neighbor', [200, 40], 300);
  const inserted = cube('inserted', [0, 0], 180);
  const catalog = new CubeNodeCatalog();
  for (const node of [source, neighbor, inserted]) catalog.add(node);

  new CubeNodeInsertAfterCoordinator(catalog).insertAfter(source, inserted);

  expect([...inserted.pos]).toEqual([264, 40]);
  expect([...neighbor.pos]).toEqual([468, 40]);
});

/** Build one first-class Cube with live graph geometry. */
function cube(instanceId: string, position: [number, number], width: number): CubeNode {
  return {
    id: instanceId,
    type: `cube-${instanceId}`,
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
