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
/** Verify the save boundary around graph-owned native Cube nodes. */

import { expect, test } from '@jest/globals';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { ComfyCubeSaveAdapter } from '../../frontend/comfyui/ui/cube/node/ComfyCubeSaveAdapter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

test('lists native Cube save records and owns identity writes', () => {
  const definition = {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'Cube: Detailer',
    _nodes: [],
    inputNode: {},
    outputNode: {},
    extra: {},
  } as unknown as NativeCubeSubgraph;
  const node: CubeNode = {
    id: 'root-node-1',
    type: definition.id,
    subgraph: definition,
    title: 'Detailer',
    pos: [100, 120],
    size: [840, 520],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: 'local/personal/Detailer.cube',
        default_alias: 'Detailer',
        instance_id: 'instance-1',
        cube_version: '1.0.0',
        cube_revision_ref: 'WORKTREE',
      },
      sugarcubes_surface: {
        schema: 1,
        revealed: true,
      },
    },
    inputs: [],
    outputs: [],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
  const catalog = new CubeNodeCatalog();
  catalog.add(node);
  const adapter = new ComfyCubeSaveAdapter({ getCatalog: () => catalog });

  expect(adapter.listInstances()).toEqual([
    expect.objectContaining({
      definitionId: definition.id,
      cubeId: 'local/personal/Detailer.cube',
      instanceId: 'instance-1',
      surfaceSize: [840, 520],
      surfaceState: { schema: 1, revealed: true },
    }),
  ]);

  expect(
    adapter.updateIdentities(['instance-1'], {
      cubeVersion: '1.1.0',
      cubeRevisionRef: 'WORKTREE',
      cubeDefinitionKey: 'local/personal/Detailer.cube@1.1.0',
    }),
  ).toBe(1);
  expect(node.properties.sugarcubes_cube).toMatchObject({
    cube_version: '1.1.0',
    cube_revision_ref: 'WORKTREE',
    cube_definition_key: 'local/personal/Detailer.cube@1.1.0',
  });
  expect(definition.extra?.sugarcubes_cube).toEqual(node.properties.sugarcubes_cube);
});

test('returns an empty boundary when the Cube runtime is unavailable', () => {
  const adapter = new ComfyCubeSaveAdapter({ getCatalog: () => null });

  expect(adapter.listInstances()).toEqual([]);
  expect(adapter.updateIdentities(['missing'], { cubeVersion: '1.0.0' })).toBe(0);
});
