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
/** Characterize marker-to-native-boundary topology translation. */

import { buildCubePayloadTopology } from '../../frontend/comfyui/ui/cube/CubePayloadTopology.js';
import type { ImportPayload } from '../../frontend/comfyui/ui/import/PlacementPayload.js';

describe('CubePayloadTopology', () => {
  test('turns marker connections into graph-owned inputs and outputs', () => {
    const payload: ImportPayload = {
      nodes: [
        { symbol: 'loader', class_type: 'LoadImage' },
        { symbol: 'sampler', class_type: 'KSampler' },
      ],
      markers: [
        { alias: 'input.image', kind: 'input', class_type: 'SugarCubes.CubeInput' },
        { alias: 'output.image', kind: 'output', class_type: 'SugarCubes.CubeOutput' },
      ],
      connections: [
        {
          kind: 'binding',
          from: { symbol: 'input.image', slot: 0 },
          to: { symbol: 'loader', input: 'image' },
        },
        {
          kind: 'link',
          from: { symbol: 'loader', slot: 1 },
          to: { symbol: 'sampler', input: 'latent_image' },
        },
        {
          kind: 'output',
          from: { symbol: 'sampler', slot: 0 },
          to: { symbol: 'output.image', input: 'value' },
        },
      ],
    };

    expect(buildCubePayloadTopology(payload)).toEqual({
      nodes: payload.nodes,
      nodeConnections: [
        {
          sourceSymbol: 'loader',
          sourceSlot: 1,
          targetSymbol: 'sampler',
          targetInput: 'latent_image',
        },
      ],
      inputs: [
        {
          id: 'input.image',
          name: 'input.image',
          label: 'input.image',
          type: null,
          targets: [{ symbol: 'loader', input: 'image' }],
        },
      ],
      outputs: [
        {
          id: 'output.image',
          name: 'output.image',
          label: 'output.image',
          type: null,
          sourceSymbol: 'sampler',
          sourceSlot: 0,
        },
      ],
    });
  });

  test('never returns marker entries as runtime nodes', () => {
    const payload: ImportPayload = {
      nodes: [{ symbol: 'node', class_type: 'Example' }],
      markers: [
        { alias: 'input.value', kind: 'input', class_type: 'SugarCubes.CubeInput' },
        { alias: 'output.value', kind: 'output', class_type: 'SugarCubes.CubeOutput' },
      ],
      connections: [],
    };

    const topology = buildCubePayloadTopology(payload);

    expect(topology.nodes).toEqual(payload.nodes);
    expect(topology.nodes).not.toContain(payload.markers?.[0]);
    expect(topology.nodes).not.toContain(payload.markers?.[1]);
  });

  test('prefers the explicit read-only boundary contract over marker-era inference', () => {
    const payload: ImportPayload = {
      nodes: [{ symbol: 'nested', class_type: 'nested-subgraph-id' }],
      boundaries: {
        inputs: [
          {
            id: 'input.image',
            name: 'input.image',
            label: 'IMAGE Input',
            type: 'IMAGE',
            targets: [{ symbol: 'nested', input: 'image' }],
          },
        ],
        outputs: [
          {
            id: 'output.image',
            name: 'output.image',
            label: 'IMAGE Output',
            type: 'IMAGE',
            source: { symbol: 'nested', slot: 0 },
          },
        ],
      },
      markers: [],
      connections: [],
    };

    expect(buildCubePayloadTopology(payload)).toMatchObject({
      inputs: [{ id: 'input.image', type: 'IMAGE', targets: [{ input: 'image' }] }],
      outputs: [{ id: 'output.image', type: 'IMAGE', sourceSymbol: 'nested' }],
    });
  });
});
