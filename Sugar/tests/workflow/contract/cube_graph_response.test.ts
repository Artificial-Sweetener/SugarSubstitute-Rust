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
/** Prove canonical graph responses cannot introduce ambiguous live-node identities or geometry. */
import { expect, test } from '@jest/globals';
import { readCubeGraphAnalysis } from '../../../frontend/comfyui/ui/workflow/infrastructure/CubeGraphResponse.js';
import { graphResponse } from '../support/CubeGraphFixtures.js';

test('projects stable instance identities, segment permissions and finite native placement', () => {
  expect(readCubeGraphAnalysis(graphResponse())).toMatchObject({
    instances: [
      { instanceId: 'first', nodeId: '1' },
      { instanceId: 'second', nodeId: '2' },
    ],
    segments: [{ instanceIds: ['first', 'second'], reorderable: true, boundaryNodeIds: [] }],
    placements: [
      { instanceId: 'first', position: [0, 0] },
      { instanceId: 'second', position: [244, 0] },
    ],
  });
});

test.each([
  ['incompatible schema', { schema_version: 2 }],
  ['missing workflow', { workflow: null }],
  [
    'ambiguous identity',
    { instances: [graphResponse().instances[0], graphResponse().instances[0]] },
  ],
  ['incomplete segment partition', { segments: [] }],
  [
    'overlapping segments',
    { segments: [graphResponse().segments[0], graphResponse().segments[0]] },
  ],
  [
    'unknown segment member',
    { segments: [{ instance_ids: ['absent'], reorderable: false, boundary_node_ids: [] }] },
  ],
  [
    'wired reorder permission',
    {
      segments: [
        { instance_ids: ['first', 'second'], reorderable: true, boundary_node_ids: ['ordinary'] },
      ],
    },
  ],
])('rejects %s before application state receives it', (_name, replacement) => {
  expect(() => readCubeGraphAnalysis({ ...graphResponse(), ...replacement })).toThrow();
});

test.each([NaN, Infinity, '244', null])(
  'rejects non-finite or coerced geometry %s',
  (coordinate) => {
    const response = graphResponse();
    response.workflow.nodes[1]!.pos = [coordinate, 0];
    expect(() => readCubeGraphAnalysis(response)).toThrow();
  },
);

test('rejects a node-id and instance-id mismatch rather than moving another native node', () => {
  const response = graphResponse();
  response.instances[0]!.node_id = '2';
  expect(() => readCubeGraphAnalysis(response)).toThrow();
});
