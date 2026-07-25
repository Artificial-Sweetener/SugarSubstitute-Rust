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
import { CubeSubgraphRegistrar } from '../../frontend/comfyui/ui/cube/CubeSubgraphRegistrar.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';

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
      hasSubgraph: () => false,
      createSubgraph,
      createNode: () => null,
    });

    const warnings = registrar.register({
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

    expect(warnings).toEqual([]);
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

  test('does not replace an already registered nested definition', () => {
    const createSubgraph = jest.fn<(data: UnknownRecord) => null>(() => null);
    const registrar = new CubeSubgraphRegistrar({
      hasSubgraph: (id) => id === 'existing',
      createSubgraph,
      createNode: () => null,
    });

    expect(registrar.register({ subgraphs: [{ id: 'existing' }] })).toEqual([]);
    expect(createSubgraph).not.toHaveBeenCalled();
  });
});
