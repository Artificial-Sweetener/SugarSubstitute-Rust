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
/** Verify focused nested-subgraph registration for imported Cubes. */

import { jest } from '@jest/globals';
import { CubeSubgraphRegistrar } from '../../../frontend/comfyui/ui/cube/CubeSubgraphRegistrar.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

describe('CubeSubgraphRegistrar', () => {
  test('normalizes and configures an absent embedded subgraph once', () => {
    const configure = jest.fn();
    let createdData: UnknownRecord | null = null;
    const createSubgraph = jest.fn<(data: UnknownRecord) => { configure: typeof configure }>(
      (data) => {
        createdData = data;
        return { configure };
      },
    );
    const registrar = new CubeSubgraphRegistrar({
      getSubgraph: () => null,
      createSubgraph,
      createNode: () => null,
      discardSubgraph: jest.fn(),
    });

    const result = registrar.register({
      nodes: [
        {
          symbol: 'nested',
          class_type: 'nested-id',
          inputs: { prompt: '' },
          layout: { title: 'Nested Editor' },
        },
      ],
      subgraphs: [
        {
          id: 'nested-id',
          version: 0.4,
          last_node_id: 0,
          last_link_id: 0,
          nodes: [],
          groups: [],
          links: [],
          extra: {},
        },
      ],
    });

    expect(result).toEqual({ warnings: [], createdIds: ['nested-id'] });
    expect(createSubgraph).toHaveBeenCalledTimes(1);
    expect(createSubgraph).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'nested-id',
        name: 'Nested Editor',
        inputNode: expect.objectContaining({ id: -10 }),
        outputNode: expect.objectContaining({ id: -20 }),
      }),
    );
    expect(configure).toHaveBeenCalledWith(createdData);
  });

  test('reconfigures an already registered definition from the current cube payload', () => {
    const configure = jest.fn();
    const createSubgraph = jest.fn<(data: UnknownRecord) => null>(() => null);
    const registrar = new CubeSubgraphRegistrar({
      getSubgraph: (id) => (id === 'existing' ? { configure } : null),
      createSubgraph,
      createNode: () => ({ widgets: [{ name: 'width', value: 512 }] }),
      discardSubgraph: jest.fn(),
    });

    expect(
      registrar.register({
        subgraphs: [
          {
            id: 'existing',
            nodes: [
              {
                id: 7,
                type: 'EmptyLatentImage',
                inputs: [{ name: 'width', link: 41, widget: { name: 'width' } }],
                widgets_values: [1080],
              },
            ],
            links: [
              {
                id: 41,
                origin_id: -10,
                origin_slot: 0,
                target_id: 7,
                target_slot: 0,
                type: 'INT',
              },
            ],
            groups: [],
          },
        ],
      }),
    ).toEqual({ warnings: [], createdIds: [] });
    expect(createSubgraph).not.toHaveBeenCalled();
    expect(configure).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'existing',
        nodes: [expect.objectContaining({ widgets_values: [1080] })],
      }),
    );
  });

  test('configures interleaved linked widgets without shifting named values', () => {
    const createdEntries: UnknownRecord[] = [];
    const registrar = new CubeSubgraphRegistrar({
      getSubgraph: () => null,
      createSubgraph: (data) => {
        createdEntries.push(data);
        return {};
      },
      createNode: () => ({
        widgets: [
          { name: 'width', value: 512 },
          { name: 'height', value: 512 },
          { name: 'resize_mode', value: 'Crop' },
          { name: 'sampling', value: 'lanczos' },
          { name: 'processor', value: 'cpu' },
          { name: 'divisible_by', value: 8 },
          { name: 'crop_position', value: 'top' },
        ],
      }),
      discardSubgraph: jest.fn(),
    });

    const result = registrar.register({
      subgraphs: [
        {
          id: 'resize-subgraph',
          nodes: [
            {
              id: 151,
              type: 'SimpleSyrup.ResizeImageToTarget',
              inputs: [
                { name: 'width', link: 3478, widget: { name: 'width' } },
                { name: 'height', link: 3482, widget: { name: 'height' } },
                { name: 'resize_mode', widget: { name: 'resize_mode' } },
                { name: 'sampling', link: 3213, widget: { name: 'sampling' } },
                { name: 'processor', widget: { name: 'processor' } },
                { name: 'divisible_by', widget: { name: 'divisible_by' } },
                { name: 'crop_position', widget: { name: 'crop_position' } },
              ],
              widgets_values: ['Keep AR', 'gpu', 2, 'center'],
            },
          ],
          links: [],
          groups: [],
        },
      ],
    });

    expect(result).toEqual({ warnings: [], createdIds: ['resize-subgraph'] });
    const createdData = createdEntries[0];
    expect(createdData).toBeDefined();
    const nodes = Array.isArray(createdData?.nodes) ? createdData.nodes : [];
    expect(nodes[0]?.widgets_values).toEqual([512, 512, 'Keep AR', 'lanczos', 'gpu', 2, 'center']);
  });
});
