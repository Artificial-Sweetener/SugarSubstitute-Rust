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
/** Verify SugarScript plans become self-contained native Cube workflows atomically. */

import { jest } from '@jest/globals';
import type {
  CubePlacementService,
  PlacedCube,
} from '../../../frontend/comfyui/ui/cube/CubePlacementService.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { NativeWorkflowMaterializer } from '../../../frontend/comfyui/ui/workflow/NativeWorkflowMaterializer.js';
import type { SugarScriptNativeWorkflowPlan } from '../../../frontend/comfyui/ui/sugarscript/SugarScriptAuthoringModels.js';

test('places, bypasses, connects, and attaches source in one batch', () => {
  const connect = jest.fn();
  const placed = [
    placedCube('source', ['output.image'], [], connect),
    placedCube('target', [], ['input.image'], jest.fn()),
  ];
  const placeBatch = jest.fn(
    (items: readonly unknown[], finalize: (value: PlacedCube[]) => void) => {
      finalize(placed);
      return placed;
    },
  );
  const graph = { extra: { retained: true } };
  const materializer = new NativeWorkflowMaterializer(
    { placeBatch } as unknown as CubePlacementService,
    graph,
    nestedDefinitions(),
  );

  const result = materializer.materializeSugarScript(plan(), 'use source\n', [100, 200]);

  expect(result.instanceIds).toEqual(['source', 'target']);
  expect(placed[0]!.node.mode).toBe(4);
  expect(connect).toHaveBeenCalledWith(0, placed[1]!.node, 0);
  // The imported Cubes form one chain from the drop point, laid out at their real sizes.
  expect([...placed[0]!.node.pos]).toEqual([100, 200]);
  expect([...placed[1]!.node.pos]).toEqual([1024, 200]);
  expect(graph.extra).toMatchObject({
    retained: true,
    sugarcubes_sugarscript: {
      schema: 1,
      state: 'synchronized',
      synchronization: {
        method: 'compiled_native_plan',
        version: 'sugarscript-native-plan-v1',
        source_semantic_hash: 'a'.repeat(64),
      },
      source: 'use source\n',
    },
  });
});

test('connects canonical Sugar output bindings to concise native surface slots', () => {
  const connect = jest.fn();
  const placed = [
    placedCube('source', ['image'], [], connect),
    placedCube('target', [], ['input.image'], jest.fn()),
  ];
  const placeBatch = jest.fn(
    (_items: readonly unknown[], finalize: (value: PlacedCube[]) => void) => {
      finalize(placed);
      return placed;
    },
  );
  const materializer = new NativeWorkflowMaterializer(
    { placeBatch } as unknown as CubePlacementService,
    { extra: {} },
    nestedDefinitions(),
  );

  materializer.materializeSugarScript(planWithCanonicalBoundaries(), 'use source\n', [0, 0]);

  expect(connect).toHaveBeenCalledWith(0, placed[1]!.node, 0);
});

test('a Cube whose nodes are not installed is refused by name, with the node pack it needs', () => {
  const connect = jest.fn();
  const source = placedCube('source', ['output.image'], [], connect);
  // Built without its nodes, the Cube has no inputs to connect.
  const target = {
    ...placedCube('target', [], [], jest.fn()),
    unavailableNodeTypes: ['Pack.A', 'Pack.B', 'Pack.C', 'Pack.D', 'Pack.E'],
  };
  const placeBatch = jest.fn(
    (_items: readonly unknown[], finalize: (value: PlacedCube[]) => void) => {
      finalize([source, target]);
      return [source, target];
    },
  );
  const materializer = new NativeWorkflowMaterializer(
    { placeBatch } as unknown as CubePlacementService,
    { extra: {} },
    nestedDefinitions(),
  );
  const workflow = plan();
  const needy = workflow.instances[1]!;
  needy.payload.cube = {
    metadata: { node_pack_requirements: [{ properties: { cnr_id: 'SomePack', ver: '1.2.3' } }] },
  } as unknown as typeof needy.payload.cube;

  expect(() => materializer.materializeSugarScript(workflow, 'use source\n', [0, 0])).toThrow(
    "Cube 'target' uses nodes that are not installed: Pack.A, Pack.B, Pack.C, Pack.D and 1 more. " +
      'It needs SomePack 1.2.3.',
  );
  expect(connect).not.toHaveBeenCalled();
});

test('restores exact companion metadata when batch finalization fails', () => {
  const previousExtra = { retained: true };
  const graph = { extra: previousExtra };
  const placeBatch = jest.fn(
    (_items: readonly unknown[], finalize: (value: PlacedCube[]) => void) => {
      finalize([
        placedCube('source', ['output.image'], [], jest.fn()),
        placedCube('target', [], [], jest.fn()),
      ]);
      throw new Error('missing boundary');
    },
  );
  const subgraphs = nestedDefinitions(['nested-definition']);
  const materializer = new NativeWorkflowMaterializer(
    { placeBatch } as unknown as CubePlacementService,
    graph,
    subgraphs,
  );

  expect(() => materializer.materializeSugarScript(plan(), 'source', [0, 0])).toThrow(
    'input.image',
  );
  expect(graph.extra).toBe(previousExtra);
  expect(subgraphs.discard).toHaveBeenCalledWith(['nested-definition']);
});

function plan(): SugarScriptNativeWorkflowPlan {
  return {
    semanticHash: 'a'.repeat(64),
    instances: [
      { instanceId: 'source', alias: 'Source', bypassed: true, payload: { cube: {} } },
      { instanceId: 'target', alias: 'Target', bypassed: false, payload: { cube: {} } },
    ],
    connections: [
      {
        sourceInstanceId: 'source',
        sourceBinding: 'output.image',
        targetInstanceId: 'target',
        targetBinding: 'input.image',
      },
    ],
  };
}

function planWithCanonicalBoundaries(): SugarScriptNativeWorkflowPlan {
  const value = plan();
  return {
    ...value,
    instances: [
      {
        ...value.instances[0]!,
        payload: {
          cube: {},
          boundaries: {
            inputs: [],
            outputs: [
              {
                id: 'output.image',
                name: 'output.image',
                label: 'Image',
                type: 'IMAGE',
                source: { symbol: 'image', slot: 0 },
              },
            ],
          },
        },
      },
      {
        ...value.instances[1]!,
        payload: {
          cube: {},
          boundaries: {
            inputs: [
              {
                id: 'input.image',
                name: 'input.image',
                label: 'Image',
                type: 'IMAGE',
                targets: [{ symbol: 'image', input: 'image' }],
              },
            ],
            outputs: [],
          },
        },
      },
    ],
  };
}

function placedCube(
  id: string,
  outputs: string[],
  inputs: string[],
  connect: jest.Mock,
): PlacedCube {
  const node = {
    id,
    title: id,
    mode: 0,
    pos: [0, 0],
    size: [900, 600],
    connect,
  } as unknown as CubeNode;
  return {
    node,
    subgraph: {
      id: `definition-${id}`,
      inputs: inputs.map((name) => ({ name, type: '*' })),
      outputs: outputs.map((name) => ({ name, type: '*' })),
    },
    warnings: [],
    internalNodeCount: 1,
  } as unknown as PlacedCube;
}

function nestedDefinitions(createdIds: string[] = []) {
  return {
    register: jest.fn(() => ({ warnings: [], createdIds })),
    discard: jest.fn(),
  };
}
