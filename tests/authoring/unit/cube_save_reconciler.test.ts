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
import { describe, expect, jest, test } from '@jest/globals';
import { CubeSaveReconciler } from '../../../frontend/comfyui/ui/save/CubeSaveReconciler.js';
import { ComfyCubeSaveAdapter } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeSaveAdapter.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

function markerNode() {
  const values = {
    cube_id: 'local/author/Model/Test.cube',
    default_alias: 'Model/Test',
    instance_alias: 'Model/Test',
  };
  return {
    id: 10,
    type: 'SugarCubes.CubeInput',
    widgets: Object.entries(values).map(([name, value]) => ({ name, value })),
    properties: {},
  };
}

describe('CubeSaveReconciler', () => {
  test('publishes, rebuilds, hydrates, installs baseline, then marks clean', async () => {
    const calls: string[] = [];
    const graph = { _nodes: [markerNode()] };
    const definition = {
      cube: {
        cube_id: 'local/author/Model/Test.cube',
        version: '1.0.0',
        surface: { controls: [{ name: 'prompt', default: 'persisted' }] },
      },
      nodes: [],
      markers: [],
      connections: [],
      layout: { groups: [] },
    };
    const entry = { status: 'ready', payload: definition, hash: 'definition-hash' };
    const definitionStore = {
      publishFinalized: jest.fn((_request, _definition) => {
        calls.push('publish');
        return entry;
      }),
    };
    const instanceManager = {
      refresh: jest.fn(() => calls.push('instances')),
    };
    const flavorService = {
      hydrateFromDefinition: jest.fn(async () => calls.push('preset')),
    };
    const dirtyManager = {
      acceptFinalizedDefinitions: jest.fn(() => calls.push('baseline')),
      addSavedIds: jest.fn(() => calls.push('known')),
      markClean: jest.fn(() => calls.push('clean')),
    };
    const reconciler = new CubeSaveReconciler({
      definitionStore,
      instanceManager,
      flavorService,
      dirtyManager,
    });

    const result = await reconciler.reconcile({
      graph,
      saved: [{ cube_id: definition.cube.cube_id, definition }],
      markerIdsByCubeId: { [definition.cube.cube_id]: [10] },
      reason: 'cube-create',
    });

    expect(calls).toEqual(['publish', 'instances', 'preset', 'baseline', 'known', 'clean']);
    expect(definitionStore.publishFinalized).toHaveBeenCalledWith(
      expect.objectContaining({
        cubeId: definition.cube.cube_id,
        cubeVersion: '1.0.0',
        definitionKey: `${definition.cube.cube_id}@1.0.0`,
      }),
      definition,
    );
    expect(flavorService.hydrateFromDefinition).toHaveBeenCalledWith(
      expect.objectContaining({ forceApply: true }),
    );
    expect(graph._nodes[0].properties).toMatchObject({
      sugarcubes_cube_version: '1.0.0',
      sugarcubes_cube_revision_ref: 'WORKTREE',
    });
    expect(result.cubeIds).toEqual([definition.cube.cube_id]);
  });

  test('rejects a successful save response without its persisted definition', async () => {
    const reconciler = new CubeSaveReconciler({
      definitionStore: { publishFinalized: jest.fn() },
    });

    await expect(
      reconciler.reconcile({
        graph: { _nodes: [] },
        saved: [{ cube_id: 'local/author/Model/Test.cube' }],
        fallbackCubeIds: ['local/author/Model/Test.cube'],
      }),
    ).rejects.toThrow('missing finalized definitions');
  });

  test('reconciles finalized identity directly onto a Cube node', async () => {
    const cubeId = 'local/author/Model/Native.cube';
    const subgraph = {
      id: 'native-definition',
      name: 'Cube: Native',
      extra: {},
    } as unknown as NativeCubeSubgraph;
    const cube: CubeNode = {
      id: 'root-node-81',
      type: subgraph.id,
      title: 'Native',
      pos: [0, 0],
      size: [720, 480],
      properties: {
        sugarcubes_kind: 'cube',
        sugarcubes_cube: {
          instance_id: 'container-81',
          cube_id: cubeId,
          default_alias: 'Native',
          cube_version: '1.0.0',
        },
        sugarcubes_surface: {},
      },
      inputs: [],
      outputs: [],
      subgraph,
      isSubgraphNode: () => true,
      connect() {},
      serialize: () => ({}),
    };
    const nodes = new CubeNodeCatalog();
    nodes.add(cube);
    const definition = {
      cube: {
        cube_id: cubeId,
        version: '1.1.0',
      },
      nodes: [],
      markers: [],
      connections: [],
      layout: { groups: [] },
    };
    const reconciler = new CubeSaveReconciler({
      definitionStore: {
        publishFinalized: jest.fn((_request, payload) => payload),
      },
      cubeNodeSave: new ComfyCubeSaveAdapter({ getCatalog: () => nodes }),
    });

    await reconciler.reconcile({
      graph: { _nodes: [] },
      saved: [{ cube_id: cubeId, definition }],
      markerIdsByCubeId: {},
      cubeNodeInstanceIdsByCubeId: { [cubeId]: ['container-81'] },
    });

    expect(cube.properties.sugarcubes_cube).toMatchObject({
      cube_id: cubeId,
      cube_version: '1.1.0',
      cube_revision_ref: 'WORKTREE',
      cube_definition_key: `${cubeId}@1.1.0`,
    });
    expect((cube.subgraph.extra as Record<string, unknown>).sugarcubes_cube).toEqual(
      cube.properties.sugarcubes_cube,
    );
  });
});
