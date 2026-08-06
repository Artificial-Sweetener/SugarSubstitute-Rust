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
/** Verify application routing behind Cube-native host affordances. */

import { jest } from '@jest/globals';
import { CubeHostAffordanceController } from '../../frontend/comfyui/ui/affordance/CubeHostAffordanceController.js';
import type { NativeGraphNode } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';
import type { CubeEditorMetadataHud } from '../../frontend/comfyui/ui/surface/CubeEditorMetadataHud.js';

test('routes draft and persisted selections to their authoritative save owners', async () => {
  const draft = cubeNode('cube_draft');
  const persisted = cubeNode('cube');
  const saveDraft = jest.fn(async () => null);
  const save = jest.fn(async () => ({ status: 'saved' }));
  const controller = controllerFor(persisted, { saveDraft, save });

  await controller.saveCube(draft);
  await controller.saveCube(persisted);

  expect(saveDraft).toHaveBeenCalledWith(
    'cube-instance',
    expect.objectContaining({ inputCount: 0, outputCount: 0 }),
  );
  expect(save).toHaveBeenCalledWith({ cubeIds: ['cube.cube'] });
});

test('clears only implementation nodes after Sugar confirmation and marks the Cube dirty', async () => {
  const cube = cubeNode('cube');
  const first = implementationNode(1);
  const second = implementationNode(2);
  cube.subgraph._nodes.push(first, second);
  const remove = jest.fn((node: unknown) => {
    const index = cube.subgraph._nodes.indexOf(node as NativeGraphNode);
    if (index >= 0) cube.subgraph._nodes.splice(index, 1);
  });
  Object.assign(cube.subgraph, { remove });
  const confirm = jest.fn(async () => true);
  const prepareGraphClear = jest.fn();
  const markGraphDirty = jest.fn();
  const announceGraphCleared = jest.fn();
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const changed = jest.fn();
  catalog.subscribe(changed);
  const contexts = new CubeEditorContextResolver(catalog);
  const controller = new CubeHostAffordanceController({
    getRuntime: () => ({ contexts, metadataHud: null, nodes: catalog }),
    cubeCreation: { saveDraft: jest.fn(async () => null) },
    cubeSave: { save: jest.fn(async () => ({ status: 'saved' })) },
    confirm: { open: confirm },
    prepareGraphClear,
    markGraphDirty,
    announceGraphCleared,
  });

  await expect(controller.clearActiveCube(cube.subgraph)).resolves.toBe(true);

  expect(confirm).toHaveBeenCalledWith(
    expect.objectContaining({
      title: 'Clear Cube implementation?',
      confirmLabel: 'Clear Cube',
    }),
  );
  expect(remove.mock.calls.map(([node]) => node)).toEqual([first, second]);
  expect(cube.subgraph.inputs).toEqual([]);
  expect(cube.subgraph.outputs).toEqual([]);
  expect(changed).toHaveBeenCalledTimes(1);
  expect(prepareGraphClear).toHaveBeenCalledTimes(1);
  expect(markGraphDirty).toHaveBeenCalledTimes(1);
  expect(announceGraphCleared).toHaveBeenCalledTimes(1);
});

test('retains an active Cube implementation when confirmation is cancelled', async () => {
  const cube = cubeNode('cube');
  const node = implementationNode(1);
  cube.subgraph._nodes.push(node);
  const remove = jest.fn();
  Object.assign(cube.subgraph, { remove });
  const confirm = jest.fn(async () => false);
  const markGraphDirty = jest.fn();
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const contexts = new CubeEditorContextResolver(catalog);
  const controller = new CubeHostAffordanceController({
    getRuntime: () => ({ contexts, metadataHud: null, nodes: catalog }),
    cubeCreation: { saveDraft: jest.fn(async () => null) },
    cubeSave: { save: jest.fn(async () => ({ status: 'saved' })) },
    confirm: { open: confirm },
    prepareGraphClear: jest.fn(),
    markGraphDirty,
    announceGraphCleared: jest.fn(),
  });

  await expect(controller.clearActiveCube(cube.subgraph)).resolves.toBe(true);
  expect(remove).not.toHaveBeenCalled();
  expect(cube.subgraph._nodes).toEqual([node]);
  expect(markGraphDirty).not.toHaveBeenCalled();
});

/** Create a controller with just enough runtime to exercise save routing. */
function controllerFor(
  cube: CubeNode,
  saves: {
    saveDraft: jest.MockedFunction<(instanceId: string, candidate: object) => Promise<null>>;
    save: jest.MockedFunction<(options: { cubeIds: string[] }) => Promise<{ status: string }>>;
  },
): CubeHostAffordanceController {
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const contexts = new CubeEditorContextResolver(catalog);
  const metadataHud = null as CubeEditorMetadataHud | null;
  return new CubeHostAffordanceController({
    getRuntime: () => ({ contexts, metadataHud, nodes: catalog }),
    cubeCreation: { saveDraft: saves.saveDraft },
    cubeSave: { save: saves.save },
    confirm: { open: jest.fn(async () => false) },
    prepareGraphClear: jest.fn(),
    markGraphDirty: jest.fn(),
    announceGraphCleared: jest.fn(),
  });
}

/** Build one draft or persisted Cube with a mutable native definition. */
function cubeNode(kind: 'cube' | 'cube_draft'): CubeNode {
  return {
    id: 'cube-node',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: kind,
      sugarcubes_cube: { instance_id: 'cube-instance', cube_id: 'cube.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

/** Build one minimal native implementation node. */
function implementationNode(id: number): NativeGraphNode {
  return {
    id,
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [],
    connect: () => null,
  };
}
