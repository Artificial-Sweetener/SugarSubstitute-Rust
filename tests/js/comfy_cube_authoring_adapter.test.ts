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
/** Characterize Cube authoring through Comfy's native conversion boundary. */

import { jest } from '@jest/globals';
import { ComfyCubeAuthoringAdapter } from '../../frontend/comfyui/ui/cube/ComfyCubeAuthoringAdapter.js';
import type { NativeCubeSubgraph } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';
import { ComfyCubeNodeFactory } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';

describe('ComfyCubeAuthoringAdapter', () => {
  test('retains and decorates Comfy generated wrapper node with native links intact', () => {
    const selected = { id: 4 };
    const subgraph = {
      id: 'definition',
      name: 'New Subgraph',
      extra: {},
      _nodes: [selected],
      inputs: [{ name: 'image', type: 'IMAGE' }],
      outputs: [{ name: 'image', type: 'IMAGE' }],
      inputNode: {},
      outputNode: {},
    } as unknown as NativeCubeSubgraph;
    const nativeNode = {
      id: 9,
      type: 'definition',
      title: 'New Subgraph',
      pos: [20, 30],
      size: [200, 100],
      properties: {} as Record<string, unknown>,
      inputs: [],
      outputs: [],
      subgraph,
      isSubgraphNode: () => true,
      title_buttons: [{ name: 'enter_subgraph' }],
      connect() {},
      serialize: () => ({}),
    };
    const subgraphs = new Map<string, NativeCubeSubgraph>();
    const graph = {
      links: new Map([
        [1, { id: 1, origin_id: 2, origin_slot: 0, target_id: 4, target_slot: 0 }],
        [2, { id: 2, origin_id: 4, origin_slot: 0, target_id: 6, target_slot: 0 }],
      ]),
    };
    const convertToSubgraph = jest.fn(() => {
      graph.links = new Map([
        [
          11,
          {
            id: 11,
            origin_id: 2,
            origin_slot: 0,
            target_id: 9,
            target_slot: 0,
            type: 'IMAGE',
          },
        ],
        [
          12,
          {
            id: 12,
            origin_id: 9,
            origin_slot: 0,
            target_id: 6,
            target_slot: 0,
            type: 'IMAGE',
          },
        ],
      ]);
      return { subgraph, node: nativeNode };
    });
    const updateSelectedItems = jest.fn();
    const nodeFactory = new ComfyCubeNodeFactory({
      graph: { add: jest.fn() },
      createNode: jest.fn(() => null),
    });
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeAuthoringAdapter({
      graph,
      subgraphs,
      convertToSubgraph,
      canvas: {
        selectedItems: new Set([selected]),
        updateSelectedItems,
      },
      nodeFactory,
      catalog,
    });

    const authored = adapter.createDraftFromSelection({
      defaultAlias: 'Test',
      instanceId: 'instance-1',
    });

    expect(convertToSubgraph).toHaveBeenCalledTimes(1);
    expect(convertToSubgraph).toHaveBeenCalledWith(new Set([selected]));
    expect(subgraphs.get('definition')).toBe(subgraph);
    expect(subgraph.name).toBe('Cube: Test');
    expect(subgraph.extra).toMatchObject({
      sugarcubes_kind: 'cube_draft',
      sugarcubes_cube: {
        instance_id: 'instance-1',
      },
    });
    expect(authored.node).toBe(nativeNode);
    expect(catalog.list()).toEqual([nativeNode]);
    expect(authored.node).toMatchObject({
      id: 9,
      pos: [20, 30],
      size: [720, 480],
    });
    expect(authored.node.properties).toMatchObject({
      sugarcubes_kind: 'cube_draft',
      sugarcubes_cube: {
        instance_id: 'instance-1',
      },
    });
    expect(graph.links).toEqual(
      new Map([
        [
          11,
          {
            id: 11,
            origin_id: 2,
            origin_slot: 0,
            target_id: 9,
            target_slot: 0,
            type: 'IMAGE',
          },
        ],
        [
          12,
          {
            id: 12,
            origin_id: 9,
            origin_slot: 0,
            target_id: 6,
            target_slot: 0,
            type: 'IMAGE',
          },
        ],
      ]),
    );
    expect(updateSelectedItems).toHaveBeenCalledTimes(1);
  });

  test('accepts a graph segment without guessing a public interface', () => {
    const selected = { id: 4 };
    const convertToSubgraph = jest.fn();
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: {
        links: new Map([[1, { id: 1, origin_id: 2, target_id: 4 }]]),
      },
      subgraphs: new Map(),
      convertToSubgraph,
      canvas: { selectedItems: new Set([selected]) },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog: new CubeNodeCatalog(),
    });

    expect(() => adapter.validateSelection()).not.toThrow();
    expect(convertToSubgraph).not.toHaveBeenCalled();
  });

  test('accepts native editor boundaries stored only on selected node slots', () => {
    const selected = {
      id: 'image-scale',
      inputs: [{ name: 'image', link: 11 }],
      outputs: [{ name: 'image', links: [12] }],
    };
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: { links: [] },
      subgraphs: new Map(),
      convertToSubgraph: jest.fn(),
      canvas: { selectedItems: new Set([selected]) },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog: new CubeNodeCatalog(),
    });

    expect(() => adapter.validateSelection()).not.toThrow();
  });

  test('uses the active native subgraph instead of the root graph for selection boundaries', () => {
    const nestedGraph = {
      links: new Map([
        [1, { id: 1, origin_id: 'input', target_id: 'image-scale' }],
        [2, { id: 2, origin_id: 'image-scale', target_id: 'output' }],
      ]),
    };
    const selected = { id: 'image-scale' };
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: { links: [] },
      subgraphs: new Map(),
      convertToSubgraph: jest.fn(),
      canvas: { graph: nestedGraph, selectedItems: new Set([selected]) },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog: new CubeNodeCatalog(),
    });

    expect(() => adapter.validateSelection()).not.toThrow();
  });

  test('rejects an empty selection before invoking Comfy conversion', () => {
    const convertToSubgraph = jest.fn();
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: {},
      subgraphs: new Map(),
      convertToSubgraph,
      canvas: { selectedItems: new Set() },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog: new CubeNodeCatalog(),
    });

    expect(() => adapter.validateSelection()).toThrow('Select at least one node');
    expect(convertToSubgraph).not.toHaveBeenCalled();
  });

  test('persists an empty draft marker on its native subgraph for Nodes 2 presentation', () => {
    const subgraph = {
      id: 'empty-definition',
      name: 'Cube: Untitled Cube',
      extra: {},
      _nodes: [],
      inputs: [{ name: 'input', type: '*' }],
      outputs: [{ name: 'output', type: '*' }],
      inputNode: {},
      outputNode: {},
    } as unknown as NativeCubeSubgraph;
    const nativeNode = {
      id: 'empty-instance',
      title: 'Untitled Cube',
      pos: [0, 0],
      size: [720, 480],
      properties: {},
      inputs: [{ name: 'input', type: '*' }],
      outputs: [{ name: 'output', type: '*' }],
      subgraph,
      isSubgraphNode: () => true,
      connect() {},
      serialize: () => ({}),
    };
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: {},
      subgraphs: new Map(),
      convertToSubgraph: jest.fn(),
      canvas: { selectedItems: new Set() },
      createEmptySubgraph: jest.fn(() => subgraph),
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => nativeNode),
      }),
      catalog: new CubeNodeCatalog(),
    });

    adapter.createEmptyDraft({
      defaultAlias: 'Untitled Cube',
      instanceId: 'empty-instance',
    });

    expect(subgraph.extra).toMatchObject({
      sugarcubes_kind: 'cube_draft',
      sugarcubes_cube: { kind: 'draft', instance_id: 'empty-instance' },
    });
  });

  test('promotes one existing subgraph without rebuilding its graph topology', () => {
    const subgraph = {
      id: 'existing-definition',
      name: 'Existing Subgraph',
      extra: {},
      _nodes: [],
      inputs: [{ name: 'image', type: 'IMAGE' }],
      outputs: [{ name: 'image', type: 'IMAGE' }],
      inputNode: {},
      outputNode: {},
    } as unknown as NativeCubeSubgraph;
    const nativeNode = {
      id: 12,
      type: 'existing-definition',
      title: 'Existing Subgraph',
      pos: [400, 120],
      size: [840, 520],
      properties: {},
      inputs: [],
      outputs: [],
      subgraph,
      isSubgraphNode: () => true,
      connect() {},
      serialize: () => ({}),
    };
    const convertToSubgraph = jest.fn();
    const catalog = new CubeNodeCatalog();
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: {},
      subgraphs: new Map(),
      convertToSubgraph,
      canvas: { selectedItems: new Set([nativeNode]) },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog,
    });

    const authored = adapter.createDraftFromSelectedSubgraph({
      defaultAlias: 'Existing',
      instanceId: 'instance-existing',
    });

    expect(convertToSubgraph).not.toHaveBeenCalled();
    expect(authored.node).toBe(nativeNode);
    expect(authored.node.pos).toEqual([400, 120]);
    expect(subgraph.name).toBe('Cube: Existing');
    expect(subgraph.extra).toMatchObject({ sugarcubes_kind: 'cube_draft' });
    expect(nativeNode.properties).toMatchObject({
      sugarcubes_kind: 'cube_draft',
      sugarcubes_cube: { instance_id: 'instance-existing' },
    });
    expect(catalog.list()).toEqual([nativeNode]);
  });

  test('rejects a selected subgraph that is already a Cube but accepts an unfinished interface', () => {
    const subgraph = {
      id: 'invalid-definition',
      name: 'Invalid',
      _nodes: [],
      inputs: [],
      outputs: [{ name: 'image', type: 'IMAGE' }],
      inputNode: {},
      outputNode: {},
    } as unknown as NativeCubeSubgraph;
    const nativeNode = {
      id: 13,
      pos: [0, 0],
      size: [720, 480],
      properties: {} as Record<string, unknown>,
      subgraph,
      isSubgraphNode: () => true,
    };
    const adapter = new ComfyCubeAuthoringAdapter({
      graph: {},
      subgraphs: new Map(),
      convertToSubgraph: jest.fn(),
      canvas: { selectedItems: new Set([nativeNode]) },
      nodeFactory: new ComfyCubeNodeFactory({
        graph: { add: jest.fn() },
        createNode: jest.fn(() => null),
      }),
      catalog: new CubeNodeCatalog(),
    });

    expect(() => adapter.validateSelectedSubgraph()).not.toThrow();
    nativeNode.properties.sugarcubes_kind = 'cube';
    expect(() => adapter.validateSelectedSubgraph()).toThrow('already a SugarCube');
  });
});
