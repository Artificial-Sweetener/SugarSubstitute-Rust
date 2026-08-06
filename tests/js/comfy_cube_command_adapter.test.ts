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
/** Verify stable Comfy command objects route Cube actions without changing ordinary behavior. */

import { jest } from '@jest/globals';
import { ComfyCubeCommandAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeCommandAdapter.js';
import { CubeAffordancePolicy } from '../../frontend/comfyui/ui/affordance/CubeAffordancePolicy.js';
import { CubeHostAffordanceController } from '../../frontend/comfyui/ui/affordance/CubeHostAffordanceController.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';
import type { CubeEditorMetadataHud } from '../../frontend/comfyui/ui/surface/CubeEditorMetadataHud.js';
import type { NativeGraphNode } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

const COMMAND_IDS = [
  'Comfy.Graph.ConvertToSubgraph',
  'Comfy.Graph.UnpackSubgraph',
  'Comfy.PublishSubgraph',
  'Comfy.Graph.EditSubgraphWidgets',
  'Comfy.SaveWorkflow',
  'Comfy.Graph.ExitSubgraph',
  'Comfy.Subgraph.SetDescription',
  'Comfy.Subgraph.SetSearchAliases',
  'Comfy.ClearWorkflow',
] as const;

interface TestCommand {
  function(metadata?: Record<string, unknown>): void | Promise<void>;
  _label?: string | (() => string);
  _menubarLabel?: string | (() => string);
}

type NativeCommand = (metadata?: Record<string, unknown>) => void;

test('routes Cube commands and retains captured native functions for ordinary Subgraphs', async () => {
  document.body.replaceChildren();
  const root = document.createElement('div');
  root.id = 'vue-app';
  document.body.append(root);
  const nativeFunctions = new Map<string, jest.MockedFunction<NativeCommand>>();
  const commands = new Map<string, TestCommand>(
    COMMAND_IDS.map((id) => {
      const native = jest.fn<NativeCommand>();
      nativeFunctions.set(id, native);
      return [id, { function: native, _label: id, _menubarLabel: id }] as const;
    }),
  );
  Object.defineProperty(root, '__vue_app__', {
    value: {
      config: {
        globalProperties: {
          $pinia: { _s: new Map([['command', { getCommand: (id: string) => commands.get(id) }]]) },
        },
      },
    },
  });
  const cube = cubeNode();
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const contexts = new CubeEditorContextResolver(catalog);
  const save = jest.fn(async () => ({ status: 'saved' }));
  const feedback = { push: jest.fn() };
  const requestSave = jest.fn(() => true);
  const setDescription = jest.fn(() => true);
  const metadataHud = {
    focus: jest.fn(),
    requestSave,
    setDescription,
  } as unknown as CubeEditorMetadataHud;
  const controller = new CubeHostAffordanceController({
    getRuntime: () => ({ contexts, metadataHud, nodes: catalog }),
    cubeCreation: { saveDraft: jest.fn(async () => null) },
    cubeSave: { save },
    confirm: { open: jest.fn(async () => false) },
    feedback,
    prepareGraphClear: jest.fn(),
    markGraphDirty: jest.fn(),
    announceGraphCleared: jest.fn(),
  });
  const canvas = {
    graph: { id: 'root' },
    selectedItems: new Set<unknown>([cube]),
    getNodeMenuOptions: () => [
      { content: 'Edit Subgraph Widgets', callback: jest.fn() },
      { content: 'Unpack Subgraph', callback: jest.fn() },
    ],
    getCanvasMenuOptions: () => [],
  };
  const adapter = new ComfyCubeCommandAdapter({
    document,
    canvas,
    contexts,
    policy: new CubeAffordancePolicy(),
    controller,
  });
  adapter.install();

  await commands.get('Comfy.PublishSubgraph')?.function();
  expect(save).toHaveBeenCalledWith({ cubeIds: ['cube.cube'] });
  commands.get('Comfy.Graph.ConvertToSubgraph')?.function();
  commands.get('Comfy.Graph.EditSubgraphWidgets')?.function();
  expect(nativeFunctions.get('Comfy.Graph.ConvertToSubgraph')).not.toHaveBeenCalled();
  expect(nativeFunctions.get('Comfy.Graph.EditSubgraphWidgets')).not.toHaveBeenCalled();
  expect(feedback.push).toHaveBeenCalled();
  const publishLabel = commands.get('Comfy.PublishSubgraph')?._label;
  expect(typeof publishLabel === 'function' ? publishLabel() : publishLabel).toBe('Save Cube');

  const ordinary = { isSubgraphNode: () => true, subgraph: {} };
  canvas.selectedItems = new Set([cube, ordinary]);
  commands.get('Comfy.Graph.EditSubgraphWidgets')?.function();
  expect(nativeFunctions.get('Comfy.Graph.EditSubgraphWidgets')).not.toHaveBeenCalled();
  canvas.selectedItems = new Set([ordinary]);
  commands.get('Comfy.Graph.ConvertToSubgraph')?.function();
  commands.get('Comfy.Graph.EditSubgraphWidgets')?.function();
  expect(nativeFunctions.get('Comfy.Graph.ConvertToSubgraph')).toHaveBeenCalledTimes(1);
  expect(nativeFunctions.get('Comfy.Graph.EditSubgraphWidgets')).toHaveBeenCalledTimes(1);

  canvas.selectedItems = new Set();
  Object.assign(canvas, { subgraph: cube.subgraph });
  commands.get('Comfy.Graph.EditSubgraphWidgets')?.function();
  commands.get('Comfy.SaveWorkflow')?.function();
  commands.get('Comfy.Subgraph.SetDescription')?.function({ description: 'Cube description' });
  commands.get('Comfy.Subgraph.SetSearchAliases')?.function();
  commands.get('Comfy.ClearWorkflow')?.function();
  expect(requestSave).toHaveBeenCalledTimes(1);
  expect(setDescription).toHaveBeenCalledWith('Cube description');
  expect(feedback.push).toHaveBeenCalledWith(
    'info',
    'SugarCube action unavailable',
    expect.stringContaining('aliases'),
  );
  expect(nativeFunctions.get('Comfy.SaveWorkflow')).not.toHaveBeenCalled();
  expect(nativeFunctions.get('Comfy.Graph.EditSubgraphWidgets')).toHaveBeenCalledTimes(1);
  expect(dynamicLabel(commands, 'Comfy.SaveWorkflow')).toBe('Save Cube');
  expect(dynamicLabel(commands, 'Comfy.Graph.ExitSubgraph')).toBe('Exit Cube');
  expect(dynamicLabel(commands, 'Comfy.ClearWorkflow')).toBe('Clear Cube implementation');

  const nestedGraph = { id: 'nested', _nodes: [] };
  cube.subgraph._nodes.push({
    id: 'nested-node',
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [],
    connect: () => null,
    isSubgraphNode: () => true,
    subgraph: nestedGraph,
  } as NativeGraphNode);
  Object.assign(canvas, { subgraph: nestedGraph });
  commands.get('Comfy.Graph.EditSubgraphWidgets')?.function();
  commands.get('Comfy.SaveWorkflow')?.function();
  expect(nativeFunctions.get('Comfy.SaveWorkflow')).toHaveBeenCalledTimes(1);
  expect(nativeFunctions.get('Comfy.Graph.EditSubgraphWidgets')).toHaveBeenCalledTimes(2);
  expect(dynamicLabel(commands, 'Comfy.Graph.ExitSubgraph')).toBe('Exit Subgraph');

  adapter.dispose();
  expect(commands.get('Comfy.PublishSubgraph')?.function).toBe(
    nativeFunctions.get('Comfy.PublishSubgraph'),
  );
});

/** Read a wrapped command's current dynamic presentation. */
function dynamicLabel(commands: ReadonlyMap<string, TestCommand>, id: string): string | undefined {
  const label = commands.get(id)?._label;
  return typeof label === 'function' ? label() : label;
}

/** Build one persisted Cube selection operand. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1', cube_id: 'cube.cube' },
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
