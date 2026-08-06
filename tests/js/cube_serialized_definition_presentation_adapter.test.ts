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
/** Verify Cube descriptions reach Comfy before it generates native node definitions. */

import { CubeSerializedDefinitionPresentationAdapter } from '../../frontend/comfyui/ui/cube/CubeSerializedDefinitionPresentationAdapter.js';

test('adapts marked definitions and preserves ordinary Subgraph descriptions', () => {
  const cube = {
    id: 'cube-definition',
    description: 'Subgraph node for Cube',
    extra: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { default_alias: '<Cube>', description: 'Literal <description>' },
    },
  };
  const draft = {
    id: 'draft-definition',
    extra: {
      sugarcubes_kind: 'cube_draft',
      sugarcubes_cube: { default_alias: 'Draft' },
    },
  };
  const ordinary = { id: 'ordinary', description: 'Ordinary Subgraph', extra: {} };
  const workflow = { definitions: { subgraphs: [cube, draft, ordinary] } };

  expect(new CubeSerializedDefinitionPresentationAdapter().prepare(workflow)).toBe(2);
  expect(cube.description).toBe('Literal <description>');
  expect(draft).toEqual(expect.objectContaining({ description: 'SugarCube: Draft' }));
  expect(ordinary.description).toBe('Ordinary Subgraph');
});
