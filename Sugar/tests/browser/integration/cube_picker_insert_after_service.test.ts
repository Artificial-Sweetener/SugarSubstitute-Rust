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
/** Verify the add button inserts through the Cube chain, the same operation the Add cubes dialog uses. */

import { expect, jest, test } from '@jest/globals';
import type { ComfyCubeRuntime } from '../../../frontend/comfyui/ui/cube/ComfyCubeRuntime.js';
import type { ConstructedCube } from '../../../frontend/comfyui/ui/cube/CubeConstructionService.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { CubePickerPlacementAdapter } from '../../../frontend/comfyui/ui/picker/CubePickerPlacementAdapter.js';
import { CubePickerInsertAfterService } from '../../../frontend/comfyui/ui/picker/CubePickerInsertAfterService.js';
import { CubeChainService } from '../../../frontend/comfyui/ui/cube/chain/CubeChainService.js';
import { ComfyCubeChainCommitHost } from '../../../frontend/comfyui/ui/cube/chain/ComfyCubeChainCommitHost.js';
import type { GraphId } from '../../../frontend/comfyui/ui/types/graph.js';

test('places beside the source, moves the later Cubes along, and leaves hard links untouched', () => {
  const f = fixture();
  const result = f.service.insert('source', 'SugarCubes.Cube.inserted');
  expect(result).toBe(f.inserted);
  expect(f.prepare).toHaveBeenCalledWith('SugarCubes.Cube.inserted');
  expect([...f.inserted.pos]).toEqual([300, 40]);
  expect([...f.downstream.pos]).toEqual([700, 80]);
  expect(f.construct).toHaveBeenCalledWith(
    expect.anything(),
    expect.objectContaining({
      instanceId: 'inserted',
      instanceAlias: 'Inserted',
      size: [340, 160],
    }),
  );
  expect(f.source.outputs[0]?.links).toBeNull();
  for (const node of [f.source, f.inserted, f.downstream])
    expect(node.connect).not.toHaveBeenCalled();
  expect(f.discardDefinitions).not.toHaveBeenCalled();
});

test('discards the staged definitions when adding the Cube to the graph fails', () => {
  const f = fixture();
  f.failAdd();
  expect(() => f.service.insert('source', 'SugarCubes.Cube.inserted')).toThrow('graph add failed');
  expect(f.discardDefinitions).toHaveBeenCalledTimes(1);
  expect(f.nodes.list()).toEqual([f.source, f.downstream]);
});

/** Bind a real chain and commit host to a graph whose only host behaviour is a node map. */
function fixture() {
  const source = cube('source', [20, 40], 220);
  const downstream = cube('downstream', [300, 80], 360);
  const inserted = cube('inserted', [0, 0], 340);
  const nodes = new CubeNodeCatalog();
  nodes.add(source);
  nodes.add(downstream);
  const root = new Map<GraphId, CubeNode>([
    [source.id, source],
    [downstream.id, downstream],
  ]);
  let addFailure = false;
  const construct = jest.fn(
    (): ConstructedCube => ({
      node: inserted,
      subgraph: inserted.subgraph,
      warnings: [],
      internalNodeCount: 2,
    }),
  );
  const discardDefinitions = jest.fn();
  const chain = new CubeChainService({
    graph: {
      get _nodes() {
        return [...root.values()];
      },
    },
    nodes,
    isCurrentRoot: () => true,
    matches: () => [],
    accepts: () => true,
    commit: new ComfyCubeChainCommitHost({
      graph: {
        add: (node) => {
          if (addFailure) throw new Error('graph add failed');
          root.set(node.id, node);
        },
        remove: (node) => {
          root.delete(node.id);
        },
        getNodeById: (id) => root.get(id),
        getLink: () => null,
      },
      nodes,
      history: { beforeChange() {}, afterChange() {}, setDirtyCanvas() {} },
      construction: { construct, discard: jest.fn() },
      definitions: {
        stage: (payload) => ({ payload, definitionIds: ['definition-inserted'] }),
        discard: discardDefinitions,
      },
      reconcile: () => undefined,
      announce: () => undefined,
    }),
  });
  const runtime = {
    chain,
    construction: { previewSize: () => [340, 160] },
  } as unknown as ComfyCubeRuntime;
  const prepare = jest.fn(() => ({
    descriptor: { cubeId: 'local/inserted.cube', displayName: 'Inserted', inputs: [{}] },
    payload: { cube: { cube_id: 'local/inserted.cube' } },
    options: { instanceAlias: 'Inserted' },
  }));
  const service = new CubePickerInsertAfterService({
    placement: { prepare } as unknown as CubePickerPlacementAdapter,
    getRuntime: () => runtime,
    createId: () => 'inserted',
  });
  return {
    service,
    source,
    downstream,
    inserted,
    nodes,
    prepare,
    construct,
    discardDefinitions,
    failAdd: () => {
      addFailure = true;
    },
  };
}

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
