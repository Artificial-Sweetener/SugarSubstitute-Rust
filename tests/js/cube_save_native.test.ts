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
/** Verify native Cube save descriptors and persisted surface state. */

import { jest, test, expect } from '@jest/globals';
import { CubeSaveService } from '../../frontend/comfyui/ui/save/CubeSaveService.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { ComfyCubeSaveAdapter } from '../../frontend/comfyui/ui/cube/node/ComfyCubeSaveAdapter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import type { ComfyApplication, ComfyGraph } from '../../frontend/comfyui/ui/types/graph.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';

test('saves one semantic Cube through its native definition and real node', async () => {
  const cubeId = 'local/personal/Detailer.cube';
  const definitionId = '11111111-1111-4111-8111-111111111111';
  const definition = {
    id: definitionId,
    name: 'Cube: Detailer',
    _nodes: [],
    inputNode: {},
    outputNode: {},
  } as unknown as NativeCubeSubgraph;
  const nodes = new CubeNodeCatalog();
  const cubeNode: CubeNode = {
    id: 'root-node-1',
    type: definition.id,
    subgraph: definition,
    title: 'Detailer',
    pos: [100, 120],
    size: [840, 520],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: cubeId,
        default_alias: 'Detailer',
        instance_id: 'instance-1',
        cube_version: '1.0.0',
        cube_revision_ref: 'WORKTREE',
        description: 'Native save description',
      },
      sugarcubes_surface: {
        schema: 1,
        revealed: false,
        selected_output: 'image',
      },
    },
    inputs: [],
    outputs: [],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
  nodes.add(cubeNode);
  const graph: ComfyGraph = { _nodes: [cubeNode], _groups: [], links: {} };
  const workflow: UnknownRecord = {
    version: 1,
    nodes: [{ id: 'root-node-1', type: definitionId }],
    links: [],
    groups: [],
    definitions: {
      subgraphs: [{ id: definitionId, nodes: [], links: [] }],
    },
    extra: {
      sugarcubes_containers: {
        schema: 1,
        items: [
          {
            id: 'instance-1',
            definition_id: definitionId,
            title: 'Detailer',
            pos: [100, 120],
            size: [840, 520],
          },
        ],
        links: [],
      },
    },
  };
  const app: ComfyApplication = {
    graph,
    graphToPrompt: () => ({
      output: {},
      workflow,
    }),
  };
  const requests: UnknownRecord[] = [];
  const saveImplementation = jest.fn(async (body: BodyInit | null) => {
    if (typeof body !== 'string') throw new Error('Expected serialized save body');
    requests.push(JSON.parse(body) as UnknownRecord);
    return {
      response: { ok: true, status: 200, statusText: '' },
      data: { saved: [{ cube_id: cubeId }] },
    };
  });
  const service = new CubeSaveService({
    adapter: { getApp: () => app, getConsole: () => console },
    api: { saveImplementation },
    dirtyManager: {
      getImplementationDirtyCubeIds: () => new Set([cubeId]),
    },
    cubeBrowser: {
      getCubes: () => [{ cube_id: cubeId, name: 'Detailer', is_writable: true }],
    },
    cubeNodeSave: new ComfyCubeSaveAdapter({ getCatalog: () => nodes }),
    saveReconciler: { reconcile: jest.fn(async () => ({ cubeIds: [cubeId], entries: [] })) },
  });

  const outcome = await service.save();

  expect(saveImplementation).toHaveBeenCalledTimes(1);
  expect(outcome).toEqual({ status: 'saved', savedCubeIds: [cubeId] });
  const savedRequest = requests[0];
  const cubes = Array.isArray(savedRequest?.cubes) ? savedRequest.cubes : [];
  expect(cubes).toEqual([
    expect.objectContaining({
      cube_id: cubeId,
      definition_id: definitionId,
      instance_node_ids: ['root-node-1'],
      description: 'Native save description',
      metadata: expect.objectContaining({
        surface_size: [840, 520],
        surface_state: {
          schema: 1,
          revealed: false,
          selected_output: 'image',
        },
      }),
    }),
  ]);
});
