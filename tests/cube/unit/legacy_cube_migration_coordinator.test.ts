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
/** Verify group-era migration restores extension-owned Cube boundaries. */

import { jest } from '@jest/globals';
import type {
  BuiltCubeGraph,
  NativeCubeSubgraph,
} from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import type {
  CubePlacementService,
  PlacedCube,
} from '../../../frontend/comfyui/ui/cube/CubePlacementService.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { LegacyCubeGraphBuilder } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeGraphBuilder.js';
import { LegacyCubeMigrationCoordinator } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeMigrationCoordinator.js';
import type {
  LegacyCubeMigrationBatch,
  LegacyCubePlan,
} from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

describe('LegacyCubeMigrationCoordinator', () => {
  test('places without history and restores links through native Cube nodes', () => {
    const batch: LegacyCubeMigrationBatch = {
      plans: [plan('first', 'First'), plan('second', 'Second')],
      warnings: [],
      connections: [
        {
          origin: { kind: 'root', nodeId: 1, slot: 0 },
          target: { kind: 'cube-input', cubeKey: 'first', name: 'input.image' },
          type: 'IMAGE',
        },
        {
          origin: { kind: 'cube-output', cubeKey: 'first', name: 'output.image' },
          target: { kind: 'cube-input', cubeKey: 'second', name: 'input.image' },
          type: 'IMAGE',
        },
      ],
    };
    const builtByKey = new Map<string, BuiltCubeGraph>([
      ['first', built('definition-first')],
      ['second', built('definition-second')],
    ]);
    const build = jest.fn((value: LegacyCubePlan) => builtByKey.get(value.key)!);
    const placedByDefinition = new Map<string, PlacedCube>([
      ['definition-first', placed('first-container', builtByKey.get('first')!.subgraph)],
      ['definition-second', placed('second-container', builtByKey.get('second')!.subgraph)],
    ]);
    const placeBuilt = jest.fn((request: { built: BuiltCubeGraph }) => {
      return placedByDefinition.get(request.built.subgraph.id)!;
    });
    const rootProducer = connectableNode(1, 0, 1);
    const logger = { error: jest.fn(), warn: jest.fn() };
    const coordinator = new LegacyCubeMigrationCoordinator({
      graphBuilder: { build } as unknown as LegacyCubeGraphBuilder,
      placement: { placeBuilt } as unknown as CubePlacementService,
      graph: {
        getNodeById: (id) => (id === 1 ? rootProducer : null),
      },
      logger,
    });

    const result = coordinator.restore(batch);

    expect(result).toEqual({ migrated: 2, connected: 2, warnings: [] });
    expect(placeBuilt).toHaveBeenCalledTimes(2);
    expect(placeBuilt).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        title: 'First',
        recordHistory: false,
        geometry: { position: [10, 20], size: [700, 500] },
      }),
    );
    const first = placedByDefinition.get('definition-first')!.node;
    const second = placedByDefinition.get('definition-second')!.node;
    expect(rootProducer.connect).toHaveBeenCalledWith(0, first, 0);
    expect(first.connect).toHaveBeenCalledWith(0, second, 0);
  });
});

function plan(key: string, title: string): LegacyCubePlan {
  return {
    key,
    cubeId: `cube-${key}`,
    cubeVersion: '1.0.0',
    title,
    metadata: {},
    position: [10, 20],
    size: [700, 500],
    nodes: [],
    groups: [],
    internalLinks: [],
    inputs: [],
    outputs: [],
  };
}

function built(id: string): BuiltCubeGraph {
  return {
    subgraph: {
      id,
      name: id,
      _nodes: [],
      inputs: [{ name: 'input.image', type: 'IMAGE' }],
      outputs: [{ name: 'output.image', type: 'IMAGE' }],
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
    } as unknown as NativeCubeSubgraph,
    nodesBySymbol: new Map(),
    warnings: [],
  };
}

function placed(id: string, subgraph: NativeCubeSubgraph): PlacedCube {
  const node = connectableNode(
    id,
    subgraph.inputs.length,
    subgraph.outputs.length,
  ) as unknown as CubeNode;
  node.type = subgraph.id;
  node.title = id;
  node.pos = [0, 0];
  node.size = [700, 500];
  node.properties = {
    sugarcubes_kind: 'cube',
    sugarcubes_cube: { instance_id: id, cube_id: `${id}.cube` },
    sugarcubes_surface: {},
  };
  node.subgraph = subgraph;
  node.isSubgraphNode = () => true;
  node.serialize = () => ({});
  return {
    node,
    subgraph,
    warnings: [],
    internalNodeCount: 1,
  };
}

/** Build a connectable native root-node double. */
function connectableNode(id: string | number, inputCount: number, outputCount: number) {
  return {
    id,
    inputs: Array.from({ length: inputCount }, () => ({})),
    outputs: Array.from({ length: outputCount }, () => ({})),
    connect: jest.fn(),
  };
}
