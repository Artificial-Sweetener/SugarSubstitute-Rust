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
import { CubeSaveReconciler } from '../../web/comfyui/ui/save/CubeSaveReconciler.js';

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
});
