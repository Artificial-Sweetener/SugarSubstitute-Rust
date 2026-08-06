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
/** Characterize Comfy's number-or-string subgraph link identity contract. */

import { describe, expect, test } from '@jest/globals';
import { normalizeSubgraphPayload } from '../../frontend/comfyui/ui/graph/SubgraphSerialization.js';

const DEFINITION_ID = '11111111-1111-4111-8111-111111111111';
const NODE_ID = '953141f7-f9a8-4ab5-8337-861a09c07ea2';

describe('normalizeSubgraphPayload', () => {
  test.each([
    {
      id: 7,
      origin_id: NODE_ID,
      origin_slot: 1,
      target_id: -20,
      target_slot: 0,
      type: 'IMAGE',
    },
    [7, NODE_ID, 1, -20, 0, 'IMAGE'],
  ])('preserves UUID node endpoints from current Comfy links', (link) => {
    const normalized = normalizeSubgraphPayload(
      {
        id: DEFINITION_ID,
        name: 'Cube: Prompt by Region',
        inputNode: { id: -10, bounding: [0, 0, 75, 100] },
        outputNode: { id: -20, bounding: [500, 0, 75, 100] },
        inputs: [],
        outputs: [{ id: 'image-output', name: 'image', type: 'IMAGE', linkIds: [7] }],
        nodes: [{ id: NODE_ID, type: 'VAEDecode' }],
        links: [link],
      },
      DEFINITION_ID,
    );

    expect(normalized?.links).toEqual([
      expect.objectContaining({
        origin_id: NODE_ID,
        origin_slot: 1,
        target_id: -20,
        target_slot: 0,
      }),
    ]);
  });

  test('preserves numeric node endpoints for ordinary legacy subgraphs', () => {
    const normalized = normalizeSubgraphPayload(
      {
        id: DEFINITION_ID,
        name: 'Ordinary Subgraph',
        inputNode: { id: -10, bounding: [0, 0, 75, 100] },
        outputNode: { id: -20, bounding: [500, 0, 75, 100] },
        inputs: [],
        outputs: [{ id: 'image-output', name: 'image', type: 'IMAGE', linkIds: [7] }],
        nodes: [{ id: 42, type: 'VAEDecode' }],
        links: [[7, 42, 0, -20, 0, 'IMAGE']],
      },
      DEFINITION_ID,
    );

    expect(normalized?.links[0]).toEqual(
      expect.objectContaining({ origin_id: 42, target_id: -20 }),
    );
  });

  test('rejects links with invalid endpoint identities instead of shrinking topology', () => {
    expect(() =>
      normalizeSubgraphPayload(
        {
          id: DEFINITION_ID,
          name: 'Malformed Subgraph',
          inputNode: { id: -10, bounding: [0, 0, 75, 100] },
          outputNode: { id: -20, bounding: [500, 0, 75, 100] },
          inputs: [],
          outputs: [{ id: 'image-output', name: 'image', type: 'IMAGE', linkIds: [7] }],
          nodes: [{ id: NODE_ID, type: 'VAEDecode' }],
          links: [
            {
              id: 7,
              origin_id: { invalid: true },
              origin_slot: 0,
              target_id: -20,
              target_slot: 0,
              type: 'IMAGE',
            },
          ],
        },
        DEFINITION_ID,
      ),
    ).toThrow('serialized link at index 0 is invalid');
  });
});
