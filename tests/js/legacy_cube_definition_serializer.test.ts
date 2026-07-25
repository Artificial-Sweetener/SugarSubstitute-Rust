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
/** Characterize native subgraph serialization for one extracted legacy Cube. */

import { LegacyCubeDefinitionSerializer } from '../../frontend/comfyui/ui/cube/migration/LegacyCubeDefinitionSerializer.js';
import type { LegacyCubePlan } from '../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

describe('LegacyCubeDefinitionSerializer', () => {
  test('preserves real node configuration and replaces marker links with graph I/O', () => {
    const plan: LegacyCubePlan = {
      key: 'instance-a',
      cubeId: 'cube-a',
      cubeVersion: '1.2.3',
      title: 'Legacy Cube',
      position: [100, 200],
      size: [700, 500],
      nodes: [
        {
          id: 11,
          type: 'Processor',
          pos: [300, 350],
          size: [240, 160],
          flags: { collapsed: true },
          order: 3,
          mode: 0,
          inputs: [
            { name: 'image', type: 'IMAGE', link: 101 },
            { name: 'unused', type: 'IMAGE', link: 999 },
          ],
          outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [102, 998] }],
          properties: { sugarcubes_symbol: 'processor', custom: 'kept' },
          widgets_values: [42, 'native'],
        },
      ],
      groups: [{ id: 6, title: 'Inner', bounding: [250, 320, 200, 180] }],
      internalLinks: [],
      inputs: [
        {
          markerId: 10,
          name: 'input.image',
          type: 'IMAGE',
          targets: [{ linkId: 101, nodeId: 11, slot: 0 }],
        },
      ],
      outputs: [
        {
          markerId: 12,
          name: 'output.image',
          type: 'IMAGE',
          source: { linkId: 102, nodeId: 11, slot: 0 },
        },
      ],
    };
    let nextId = 0;
    const serializer = new LegacyCubeDefinitionSerializer(() => `io-${(nextId += 1)}`);

    const definition = serializer.serialize(plan, 'definition-a');

    expect(definition).toMatchObject({
      id: 'definition-a',
      name: 'Cube: Legacy Cube',
      version: 1,
      inputs: [
        {
          id: 'io-1',
          name: 'input.image',
          localized_name: 'input.image',
          type: 'IMAGE',
          linkIds: [101],
        },
      ],
      outputs: [
        {
          id: 'io-2',
          name: 'output.image',
          localized_name: 'output.image',
          type: 'IMAGE',
          linkIds: [102],
        },
      ],
      links: [
        {
          id: 101,
          origin_id: -10,
          origin_slot: 0,
          target_id: 11,
          target_slot: 0,
          type: 'IMAGE',
        },
        {
          id: 102,
          origin_id: 11,
          origin_slot: 0,
          target_id: -20,
          target_slot: 0,
          type: 'IMAGE',
        },
      ],
    });
    expect(definition.nodes).toEqual([
      expect.objectContaining({
        id: 11,
        type: 'Processor',
        pos: [200, 150],
        flags: { collapsed: true },
        properties: { sugarcubes_symbol: 'processor', custom: 'kept' },
        widgets_values: [42, 'native'],
        inputs: [
          { name: 'image', type: 'IMAGE', link: 101 },
          { name: 'unused', type: 'IMAGE', link: null },
        ],
        outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [102] }],
      }),
    ]);
    expect(definition.groups).toEqual([{ id: 6, title: 'Inner', bounding: [150, 120, 200, 180] }]);
    expect(
      (definition.nodes as Array<{ type: string }>).some((node) =>
        node.type.startsWith('SugarCubes.Cube'),
      ),
    ).toBe(false);
  });
});
