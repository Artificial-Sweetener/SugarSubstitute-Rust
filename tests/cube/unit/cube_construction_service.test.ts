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
/** Verify Cube construction remains detached and compensates failed definitions. */

import { describe, expect, jest, test } from '@jest/globals';
import type {
  ComfyCubeGraphBuilder,
  NativeCubeSubgraph,
} from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { CubeConstructionService } from '../../../frontend/comfyui/ui/cube/CubeConstructionService.js';
import {
  ComfyCubeNodeFactory,
  type CubeNode,
} from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

const noopNodePackMetadata = { apply: () => false };

describe('CubeConstructionService', () => {
  test('returns a configured real Cube without inserting it into any graph', () => {
    const subgraph = nativeSubgraph('definition');
    const node = nativeSubgraphNode(subgraph);
    const discard = jest.fn();
    const service = new CubeConstructionService({
      graphBuilder: builderReturning(subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: () => node,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard },
      resolveInitialSize: () => [920, 600],
      createInstanceId: () => 'runtime-instance',
    });

    const constructed = service.construct({
      cube: { cube_id: 'local/demo.cube', version: '1.0.0', default_alias: 'Demo' },
      document: canonicalDocument('local/demo.cube', '1.0.0'),
      layout: { origin: [30, 40], groups: [] },
    });

    expect(constructed.node).toBe(node);
    expect(node.graph).toBeUndefined();
    expect(node.id).toBe('runtime-instance');
    expect(node.pos).toEqual([30, 40]);
    expect(subgraph.extra).toMatchObject({
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { cube_id: 'local/demo.cube', instance_id: 'runtime-instance' },
      sugarcubes_document: canonicalDocument('local/demo.cube', '1.0.0'),
    });
    expect(discard).not.toHaveBeenCalled();
  });

  test('discards the fresh native definition when node construction fails', () => {
    const subgraph = nativeSubgraph('definition');
    const discard = jest.fn();
    const service = new CubeConstructionService({
      graphBuilder: builderReturning(subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: () => null,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard },
      resolveInitialSize: () => [920, 600],
      createInstanceId: () => 'runtime-instance',
    });

    expect(() => service.construct({ cube: { cube_id: 'local/demo.cube' } })).toThrow(
      'real Comfy subgraph node',
    );
    expect(discard).toHaveBeenCalledWith(subgraph);
  });

  test('creates independent definitions and instance identities for repeated picker selections', () => {
    const firstDefinition = nativeSubgraph('definition-one');
    const secondDefinition = nativeSubgraph('definition-two');
    const definitions = [firstDefinition, secondDefinition];
    const nodes = new Map([
      [firstDefinition.id, nativeSubgraphNode(firstDefinition)],
      [secondDefinition.id, nativeSubgraphNode(secondDefinition)],
    ]);
    const instanceIds = ['instance-one', 'instance-two'];
    const service = new CubeConstructionService({
      graphBuilder: {
        build: jest.fn(() => {
          const subgraph = definitions.shift();
          if (!subgraph) throw new Error('Unexpected third construction.');
          return { subgraph, nodesBySymbol: new Map(), warnings: [] };
        }),
      } as unknown as ComfyCubeGraphBuilder,
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: (type) => nodes.get(type) ?? null,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard: jest.fn() },
      resolveInitialSize: () => [920, 600],
      createInstanceId: () => instanceIds.shift() ?? 'unexpected-instance',
    });

    const first = service.construct({ cube: { cube_id: 'local/demo.cube', version: '1.0.0' } });
    const second = service.construct({ cube: { cube_id: 'local/demo.cube', version: '1.0.0' } });

    expect(first.subgraph).not.toBe(second.subgraph);
    expect(first.node).not.toBe(second.node);
    expect(first.node.id).toBe('instance-one');
    expect(second.node.id).toBe('instance-two');
    expect(first.subgraph.extra?.sugarcubes_cube).toMatchObject({ instance_id: 'instance-one' });
    expect(second.subgraph.extra?.sugarcubes_cube).toMatchObject({ instance_id: 'instance-two' });
  });

  test('preserves caller-owned geometry when constructing a prebuilt definition', () => {
    const subgraph = nativeSubgraph('converted-definition');
    const node = nativeSubgraphNode(subgraph);
    const service = new CubeConstructionService({
      graphBuilder: builderReturning(subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: () => node,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard: jest.fn() },
      resolveInitialSize: () => [920, 600],
      createInstanceId: () => 'unused-instance',
    });

    service.constructBuilt({
      built: { subgraph, nodesBySymbol: new Map(), warnings: [] },
      title: 'Converted Cube',
      identity: {
        cubeId: 'local/converted.cube',
        cubeVersion: '1.0.0',
        instanceId: 'converted-instance',
        defaultAlias: 'Converted Cube',
        instanceAlias: 'Converted Cube',
        metadata: {},
      },
      geometry: { position: [125, 250], size: [735, 415] },
    });

    expect(node.pos).toEqual([125, 250]);
    expect(node.size).toEqual([735, 415]);
  });

  test('uses fresh-size policy instead of authored or persisted artifact bounds', () => {
    const subgraph = nativeSubgraph('fresh-definition');
    subgraph.inputs.push({} as never);
    subgraph.outputs.push({} as never);
    const node = nativeSubgraphNode(subgraph);
    const resolveInitialSize = jest.fn((): [number, number] => [1_040, 600]);
    const service = new CubeConstructionService({
      graphBuilder: builderReturning(subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: () => node,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard: jest.fn() },
      resolveInitialSize,
      createInstanceId: () => 'fresh-instance',
    });

    service.construct({
      cube: {
        cube_id: 'local/fresh.cube',
        metadata: {
          surface_size: [2_400, 1_800],
          surface_state: { preview: { visible: true, width: 440 } },
        },
      },
      layout: {
        groups: [{ bounding: [-100, -100, 3_200, 2_100], sugarcubes: { managed: true } }],
      },
    });

    expect(resolveInitialSize).toHaveBeenCalledWith({
      surface: { preview: { visible: true, width: 440 } },
      hasInputs: true,
    });
    expect(node.size).toEqual([1_040, 600]);
  });

  test('accepts explicit instance geometry and surface state for version replacement', () => {
    const subgraph = nativeSubgraph('replacement-definition');
    const node = nativeSubgraphNode(subgraph);
    const resolveInitialSize = jest.fn((): [number, number] => [920, 600]);
    const service = new CubeConstructionService({
      graphBuilder: builderReturning(subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        createNode: () => node,
        nodePackMetadata: noopNodePackMetadata,
      }),
      definitions: { discard: jest.fn() },
      resolveInitialSize,
      createInstanceId: () => 'unused-instance',
    });

    service.construct(
      { cube: { cube_id: 'local/demo.cube', version: '2.0.0', default_alias: 'Demo' } },
      {
        instanceId: 'stable-instance',
        instanceAlias: 'My Demo',
        position: [125, 250],
        size: [735, 415],
        surface: { schema: 3, node_order: [] },
      },
    );

    expect(node.id).toBe('stable-instance');
    expect(node.title).toBe('My Demo');
    expect(node.pos).toEqual([125, 250]);
    expect(node.size).toEqual([735, 415]);
    expect(node.properties.sugarcubes_surface).toEqual({ schema: 3, node_order: [] });
    expect(resolveInitialSize).not.toHaveBeenCalled();
  });
});

/** Build portable content retained beside its native Comfy projection. */
function canonicalDocument(cubeId: string, version: string): Record<string, unknown> {
  return {
    cube_id: cubeId,
    version,
    description: '',
    metadata: {},
    implementation: {
      nodes: {},
      inputs: {},
      outputs: {},
      layout: {},
      definitions: {},
      subgraphs: [],
    },
    surface: { default_flavor_id: 'default', controls: [] },
    flavors: { authored: [{ id: 'default', name: 'Default', values: {} }] },
  };
}

/** Build one graph-builder double whose definition is already registered. */
function builderReturning(subgraph: NativeCubeSubgraph): ComfyCubeGraphBuilder {
  return {
    build: jest.fn(() => ({ subgraph, nodesBySymbol: new Map(), warnings: [] })),
  } as unknown as ComfyCubeGraphBuilder;
}

/** Build a minimal native definition for detached construction. */
function nativeSubgraph(id: string): NativeCubeSubgraph {
  return {
    id,
    name: id,
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
  } as unknown as NativeCubeSubgraph;
}

/** Build a detached native SubgraphNode test double. */
function nativeSubgraphNode(subgraph: NativeCubeSubgraph): CubeNode {
  return {
    id: -1,
    type: subgraph.id,
    title: subgraph.name,
    pos: [0, 0],
    size: [200, 100],
    properties: {},
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    configure: jest.fn(),
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}
