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
      inputNode: {},
      outputNode: {},
    } as unknown as NativeCubeSubgraph;
    const nativeNode = {
      id: 9,
      type: 'definition',
      title: 'New Subgraph',
      pos: [20, 30],
      size: [200, 100],
      properties: {},
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

    const authored = adapter.createFromSelection({
      cubeId: 'local/personal/Test.cube',
      defaultAlias: 'Test',
      instanceId: 'instance-1',
      targetModel: '',
      supportedModels: [],
      description: '',
    });

    expect(convertToSubgraph).toHaveBeenCalledTimes(1);
    expect(convertToSubgraph).toHaveBeenCalledWith(new Set([selected]));
    expect(subgraphs.get('definition')).toBe(subgraph);
    expect(subgraph.name).toBe('Cube: Test');
    expect(subgraph.extra).toMatchObject({
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: 'local/personal/Test.cube',
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
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        cube_id: 'local/personal/Test.cube',
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

  test('rejects a selection without both graph-owned input and output boundaries', () => {
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

    expect(() => adapter.validateSelection()).toThrow(
      'requires at least one graph input and one graph output',
    );
    expect(convertToSubgraph).not.toHaveBeenCalled();
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
});
