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
/** Verify legacy non-node containers migrate once into native Cube nodes. */

import { jest } from '@jest/globals';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import {
  LegacyCubeContainerMigrationAdapter,
  type LegacyCubeContainerMigrationGraph,
} from '../../frontend/comfyui/ui/cube/migration/LegacyCubeContainerMigrationAdapter.js';

describe('LegacyCubeContainerMigrationAdapter', () => {
  test('creates real Cube nodes and restores old boundary links as native links', () => {
    const subgraph = nativeSubgraph();
    const origin = connectableNode('origin', 0, 1);
    const target = connectableNode('target', 1, 0);
    const graph: LegacyCubeContainerMigrationGraph = {
      subgraphs: new Map([[subgraph.id, subgraph]]),
      add: jest.fn(),
      onConfigure: jest.fn(),
      onSerialize: jest.fn(),
      getNodeById: jest.fn((id: string | number) =>
        String(id) === '12' ? origin : String(id) === '27' ? target : null,
      ),
    };
    const cube = cubeNode(subgraph);
    const create = jest.fn(() => cube);
    const nodes = new CubeNodeCatalog();
    const adapter = new LegacyCubeContainerMigrationAdapter({
      graph,
      factory: { create },
      nodes,
      logger: console,
    });
    const workflow = legacyWorkflow();

    graph.onConfigure?.(workflow);

    expect(create).toHaveBeenCalledWith({
      subgraph,
      instanceId: 'legacy-cube',
      title: 'Legacy Cube',
      position: [40, 60],
      size: [800, 520],
      identity: { cube_id: 'local/legacy.cube', instance_id: 'legacy-cube' },
      surface: { schema: 1, revealed: true },
    });
    expect(graph.add).toHaveBeenCalledWith(cube);
    expect(origin.connect).toHaveBeenCalledWith(0, cube, 0);
    expect(cube.connect).toHaveBeenCalledWith(0, target, 0);

    const serialized = legacyWorkflow();
    graph.onSerialize?.(serialized);
    expect(serialized.extra).not.toHaveProperty('sugarcubes_containers');
    adapter.dispose();
  });

  test('reuses an already restored native Cube instead of duplicating it', () => {
    const subgraph = nativeSubgraph();
    const cube = cubeNode(subgraph);
    const nodes = new CubeNodeCatalog();
    nodes.add(cube);
    const create = jest.fn(() => cube);
    const graph: LegacyCubeContainerMigrationGraph = {
      subgraphs: new Map([[subgraph.id, subgraph]]),
      add: jest.fn(),
      getNodeById: jest.fn(() => null),
    };
    const adapter = new LegacyCubeContainerMigrationAdapter({
      graph,
      factory: { create },
      nodes,
      logger: console,
    });

    graph.onConfigure?.(legacyWorkflow(false));

    expect(create).not.toHaveBeenCalled();
    adapter.dispose();
  });
});

/** Build the retired persistence envelope exactly as prior releases wrote it. */
function legacyWorkflow(includeLinks = true) {
  return {
    extra: {
      sugarcubes_containers: {
        schema: 1,
        items: [
          {
            id: 'legacy-cube',
            definition_id: 'legacy-definition',
            title: 'Legacy Cube',
            pos: [40, 60],
            size: [800, 520],
            identity: { cube_id: 'local/legacy.cube', instance_id: 'legacy-cube' },
            surface: { schema: 1, revealed: true },
          },
        ],
        links: includeLinks
          ? [
              {
                id: 'in-link',
                type: 'MODEL',
                origin: { kind: 'node', nodeId: '12', slot: 0 },
                target: { kind: 'cube', containerId: 'legacy-cube', slot: 0 },
              },
              {
                id: 'out-link',
                type: 'IMAGE',
                origin: { kind: 'cube', containerId: 'legacy-cube', slot: 0 },
                target: { kind: 'node', nodeId: '27', slot: 0 },
              },
            ]
          : [],
      },
    },
  };
}

/** Build one registered definition retained by the retired workflow envelope. */
function nativeSubgraph() {
  return {
    id: 'legacy-definition',
    name: 'Legacy Cube',
    _nodes: [],
    inputs: [{ name: 'model', type: 'MODEL' }],
    outputs: [{ name: 'image', type: 'IMAGE' }],
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
  };
}

/** Build the real node returned by the focused factory boundary. */
function cubeNode(subgraph: ReturnType<typeof nativeSubgraph>): CubeNode {
  return {
    id: 'legacy-cube',
    type: subgraph.id,
    title: subgraph.name,
    pos: [40, 60],
    size: [800, 520],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'legacy-cube' },
      sugarcubes_surface: {},
    },
    inputs: [{ name: 'model', type: 'MODEL' }],
    outputs: [{ name: 'image', type: 'IMAGE' }],
    subgraph,
    isSubgraphNode: () => true,
    connect: jest.fn(),
    serialize: () => ({}),
  };
}

/** Build one ordinary root node with typed slot capacity. */
function connectableNode(id: string, inputCount: number, outputCount: number) {
  return {
    id,
    inputs: Array.from({ length: inputCount }, () => ({})),
    outputs: Array.from({ length: outputCount }, () => ({})),
    connect: jest.fn(),
  };
}
