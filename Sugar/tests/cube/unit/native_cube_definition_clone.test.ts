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
/** Prove copied native contents receive isolated widget identities without changing Cube meaning. */
import { expect, test } from '@jest/globals';
import { cloneNativeCubeDefinition } from '../../../frontend/comfyui/ui/cube/graph/NativeCubeDefinitionClone.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('remaps native record and tuple links, promoted references and previews while retaining literal contents', () => {
  const source: UnknownRecord = {
    nodes: [
      {
        id: 'a',
        properties: { sugarcubes_symbol: 'positive', previewExposures: [{ nodeId: 'b' }] },
        widgets_values: ['b', '<b>literal</b>'],
      },
      { id: 'b', type: 'Patch', inputs: [{ link: 2 }] },
    ],
    links: [{ id: 1, origin_id: 'a', target_id: 'b' }, [2, -10, 0, 'b', 0, 'MODEL']],
    widgets: [{ nodeId: 'a', name: 'value' }],
  };
  let id = 0;
  const result = cloneNativeCubeDefinition({ asSerialisable: () => source }, () => `new-${++id}`);
  expect(result).toMatchObject({
    nodes: [
      {
        id: 'new-1',
        properties: { sugarcubes_symbol: 'positive', previewExposures: [{ nodeId: 'new-2' }] },
        widgets_values: ['b', '<b>literal</b>'],
      },
      { id: 'new-2', inputs: [{ link: 2 }] },
    ],
    links: [{ id: 1, origin_id: 'new-1', target_id: 'new-2' }, [2, -10, 0, 'new-2', 0, 'MODEL']],
    widgets: [{ nodeId: 'new-1', name: 'value' }],
  });
  expect(source.nodes).toMatchObject([{ id: 'a' }, { id: 'b' }]);
});

const invalidIdentities: [UnknownRecord, () => string, string][] = [
  [{ nodes: [{ id: 'a' }, { id: 'a' }] }, () => 'new', 'duplicated'],
  [{ nodes: [{ id: 'a' }] }, () => 'a', 'unavailable'],
  [{ nodes: [{ id: 'a' }, { id: 'b' }] }, () => 'new', 'unavailable'],
  [{ nodes: [{ id: {} }] }, () => 'new', 'invalid'],
];

test.each(invalidIdentities)(
  'rejects ambiguous or colliding native node identities: %s',
  (data, createId, reason) => {
    expect(() => cloneNativeCubeDefinition({ asSerialisable: () => data }, createId)).toThrow(
      reason,
    );
  },
);
