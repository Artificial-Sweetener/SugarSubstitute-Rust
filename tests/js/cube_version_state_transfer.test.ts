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
import { describe, expect, jest, test } from '@jest/globals';
import { transferCubeVersionState } from '../../frontend/comfyui/ui/cube/version/CubeVersionStateTransfer.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeGraphNode } from '../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

function internal(id: string, symbol: string, values: Record<string, unknown>): NativeGraphNode {
  return {
    id,
    pos: [0, 0],
    size: [100, 100],
    properties: { sugarcubes_symbol: symbol },
    inputs: [],
    outputs: [],
    widgets: Object.entries(values).map(([name, value]) => ({ name, value })),
    connect: jest.fn(),
  };
}

function cube(nodes: NativeGraphNode[], surface: Record<string, unknown>): CubeNode {
  return {
    id: 'instance',
    pos: [0, 0],
    size: [400, 300],
    properties: { sugarcubes_kind: 'cube', sugarcubes_cube: {}, sugarcubes_surface: surface },
    inputs: [],
    outputs: [{ name: 'image' }],
    subgraph: { _nodes: nodes } as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect: jest.fn(),
  };
}

describe('Cube version state transfer', () => {
  test('keeps matching live fields while retaining target-only authored values', () => {
    const sourceNode = internal('old-a', 'sampler', { width: 768, removed: 'source' });
    const targetNode = internal('new-a', 'sampler', { width: 512, added: 'target' });
    const source = cube([sourceNode], {
      schema: 3,
      node_order: ['old-a'],
      cards: {
        'old-a': { authored_bypass: true, revealed: true, enabled_override: null, active_mode: 0 },
      },
      minimum_column_width: 300,
      gap: 16,
      preview: { visible: true, width: 400, selected_output: 'image' },
    });
    const target = cube([targetNode], {});

    transferCubeVersionState(source, target);

    expect(targetNode.widgets?.map((widget) => widget.value)).toEqual([768, 'target']);
    expect(target.properties.sugarcubes_surface).toMatchObject({
      node_order: ['new-a'],
      cards: { 'new-a': { revealed: true } },
      minimum_column_width: 300,
      gap: 16,
      preview: { selected_output: 'image' },
    });
  });
});
