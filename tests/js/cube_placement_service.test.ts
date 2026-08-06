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
/** Verify imported placement creates one real Cube node with internal graph children. */

import { jest } from '@jest/globals';
import type {
  ComfyCubeGraphBuilder,
  NativeCubeSubgraph,
} from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { CubeConstructionService } from '../../frontend/comfyui/ui/cube/CubeConstructionService.js';
import { CubePlacementService } from '../../frontend/comfyui/ui/cube/CubePlacementService.js';
import {
  ComfyCubeNodeFactory,
  type CubeNode,
} from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

describe('CubePlacementService', () => {
  test('adds one real Cube node while children remain in its native definition', () => {
    const subgraph = nativeSubgraph('definition', ['inner-a', 'inner-b']);
    const beforeChange = jest.fn();
    const afterChange = jest.fn();
    const setDirtyCanvas = jest.fn();
    const build = jest.fn(() => ({
      subgraph,
      nodesBySymbol: new Map([
        ['a', subgraph._nodes[0]!],
        ['b', subgraph._nodes[1]!],
      ]),
      warnings: [],
    }));
    const add = jest.fn();
    const createNode = jest.fn(() => nativeSubgraphNode(subgraph));
    const catalog = new CubeNodeCatalog();
    const graphBuilder = { build } as unknown as ComfyCubeGraphBuilder;
    const service = new CubePlacementService({
      construction: constructionFor(graphBuilder, new ComfyCubeNodeFactory({ createNode })),
      graph: { add },
      catalog,
      history: { beforeChange, afterChange, setDirtyCanvas },
    });

    const placed = service.place({
      cube: {
        cube_id: 'example.cube',
        version: '1.0.0',
        default_alias: 'Example Cube',
        target_model: 'SDXL',
      },
      layout: {
        origin: [40, 60],
        groups: [
          {
            bounding: [0, 0, 1200, 700],
            sugarcubes: { managed: true, instance_id: 'instance-1' },
          },
        ],
      },
    });

    expect(add).toHaveBeenCalledWith(placed.node);
    expect(catalog.list()).toEqual([placed.node]);
    expect(createNode).toHaveBeenCalledWith('definition');
    expect(placed.node.pos).toEqual([40, 60]);
    expect(placed.node.size).toEqual([920, 600]);
    expect(placed.node.connect).toBeDefined();
    expect(placed.node.serialize).toBeDefined();
    expect(placed.node.isSubgraphNode()).toBe(true);
    expect(placed.internalNodeCount).toBe(2);
    expect(beforeChange).toHaveBeenCalledTimes(1);
    expect(afterChange).toHaveBeenCalledTimes(1);
    expect(setDirtyCanvas).toHaveBeenCalledWith(true, true);
    expect(build).toHaveBeenCalledWith(expect.any(Object), 'Cube: Example Cube');
    expect(subgraph.extra).toMatchObject({
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: 'example.cube',
        cube_version: '1.0.0',
        target_model: 'SDXL',
        instance_alias: 'Example Cube',
      },
    });
    expect(subgraph.extra?.sugarcubes_cube).not.toMatchObject({
      instance_id: 'instance-1',
    });
  });

  test('restores face state without inheriting persisted artifact frame geometry', () => {
    const subgraph = nativeSubgraph('definition');
    const graphBuilder = {
      build: () => ({ subgraph, nodesBySymbol: new Map(), warnings: [] }),
    } as unknown as ComfyCubeGraphBuilder;
    const service = new CubePlacementService({
      construction: constructionFor(graphBuilder, nodeFactoryFor(subgraph)),
      graph: { add: jest.fn() },
      catalog: new CubeNodeCatalog(),
      history: {},
    });

    const placed = service.place({
      cube: {
        cube_id: 'local/personal/example.cube',
        default_alias: 'Example',
        metadata: {
          surface_size: [760, 440],
          surface_state: { schema: 1, revealed: false },
        },
      },
      layout: {
        origin: [20, 30],
        groups: [{ bounding: [0, 0, 1200, 700], sugarcubes: { instance_id: 'cube-1' } }],
      },
    });

    expect(placed.node.size).toEqual([920, 600]);
    expect(placed.node.properties.sugarcubes_surface).toEqual({
      schema: 1,
      revealed: false,
    });
    expect(subgraph.extra?.sugarcubes_cube).toMatchObject({
      surface_size: [760, 440],
      surface_state: { revealed: false },
    });
    expect(placed.node.id).not.toBe('cube-1');
  });

  test('standardizes a thin artifact on the fresh-instance frame policy', () => {
    const subgraph = nativeSubgraph('definition');
    const graphBuilder = {
      build: () => ({ subgraph, nodesBySymbol: new Map(), warnings: [] }),
    } as unknown as ComfyCubeGraphBuilder;
    const service = new CubePlacementService({
      construction: constructionFor(graphBuilder, nodeFactoryFor(subgraph)),
      graph: { add: jest.fn() },
      catalog: new CubeNodeCatalog(),
      history: {},
    });

    const placed = service.place({
      cube: {
        cube_id: 'local/personal/thin.cube',
        default_alias: 'Thin',
        metadata: { surface_size: [320, 180] },
      },
    });

    expect(placed.node.size).toEqual([920, 600]);
  });

  test('allocates a fresh surface instance every time the same Cube is placed', () => {
    let buildIndex = 0;
    let currentSubgraph = nativeSubgraph('initial');
    const add = jest.fn();
    const graphBuilder = {
      build: () => {
        buildIndex += 1;
        currentSubgraph = nativeSubgraph(`definition-${String(buildIndex)}`);
        return {
          subgraph: currentSubgraph,
          nodesBySymbol: new Map(),
          warnings: [],
        };
      },
    } as unknown as ComfyCubeGraphBuilder;
    const service = new CubePlacementService({
      construction: constructionFor(
        graphBuilder,
        new ComfyCubeNodeFactory({
          createNode: () => nativeSubgraphNode(currentSubgraph),
        }),
      ),
      graph: { add },
      catalog: new CubeNodeCatalog(),
      history: {},
    });
    const payload = {
      cube: {
        cube_id: 'sdxl/text-to-image.cube',
        default_alias: 'SDXL/Text to Image',
      },
      layout: {
        groups: [{ sugarcubes: { instance_id: 'exported-template-instance' } }],
      },
    };

    const first = service.place(payload);
    const second = service.place(payload);

    expect(first.node.id).not.toBe(second.node.id);
    expect(first.node.id).not.toBe('exported-template-instance');
    expect(second.node.id).not.toBe('exported-template-instance');
    expect(add).toHaveBeenNthCalledWith(1, first.node);
    expect(add).toHaveBeenNthCalledWith(2, second.node);
  });
});

/** Build the minimum native definition surface needed by placement. */
function nativeSubgraph(id: string, nodeIds: string[] = []): NativeCubeSubgraph {
  return {
    id,
    name: id,
    _nodes: nodeIds.map((nodeId) => ({ id: nodeId })),
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

/** Bind a native-node factory to one registered definition. */
function nodeFactoryFor(subgraph: NativeCubeSubgraph): ComfyCubeNodeFactory {
  return new ComfyCubeNodeFactory({
    createNode: () => nativeSubgraphNode(subgraph),
  });
}

/** Bind deterministic detached construction to one graph-builder test double. */
function constructionFor(
  graphBuilder: ComfyCubeGraphBuilder,
  nodeFactory: ComfyCubeNodeFactory,
): CubeConstructionService {
  let instanceIndex = 0;
  return new CubeConstructionService({
    graphBuilder,
    nodeFactory,
    definitions: { discard: jest.fn() },
    resolveInitialSize: () => [920, 600],
    createInstanceId: () => `runtime-instance-${String(++instanceIndex)}`,
  });
}

/** Build the minimum real SubgraphNode surface used by placement. */
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
    connect() {},
    serialize: () => ({}),
    setSize(size) {
      this.size[0] = size[0] ?? 0;
      this.size[1] = size[1] ?? 0;
    },
  };
}
