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
import { expect, test } from '@jest/globals';
import { remapCubeEmbeddedSubgraphs } from '../../frontend/comfyui/ui/cube/version/CubeEmbeddedSubgraphRemapper.js';

test('isolates embedded definition IDs and every wrapper reference without mutating the artifact', () => {
  const payload = {
    nodes: [{ symbol: 'outer', class_type: 'authored-subgraph' }],
    subgraphs: [
      {
        id: 'authored-subgraph',
        nodes: [{ id: 1, type: 'nested-subgraph' }],
      },
      { id: 'nested-subgraph', nodes: [] },
    ],
  };
  const ids = ['runtime-outer', 'runtime-nested'];

  const remapped = remapCubeEmbeddedSubgraphs(payload, () => ids.shift() ?? 'unexpected');

  expect(payload.nodes[0]?.class_type).toBe('authored-subgraph');
  expect(remapped.definitionIds).toEqual(['runtime-outer', 'runtime-nested']);
  expect(remapped.payload.nodes?.[0]?.class_type).toBe('runtime-outer');
  expect(remapped.payload.subgraphs?.[0]).toMatchObject({
    id: 'runtime-outer',
    nodes: [{ id: 1, type: 'runtime-nested' }],
  });
  expect(remapped.payload.subgraphs?.[1]?.id).toBe('runtime-nested');
});
