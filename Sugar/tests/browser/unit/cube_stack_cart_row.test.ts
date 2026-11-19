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
/** Prove a cart row reads a Cube the way the stack rail does. */
import { expect, test } from '@jest/globals';
import { cubeStackCartRow } from '../../../frontend/comfyui/ui/picker/CubeStackCartRow.js';

test('a cart row shows the model as its chip and the name without the model route', () => {
  const row = cubeStackCartRow(
    'key',
    { cube_id: 'Someone/Pack/SDXL/Text to Image.cube', cube_version: '2', target_model: 'SDXL' },
    'SDXL/Text to Image',
    false,
  );
  expect(row.label).toBe('Text to Image');
  expect(row.targetModel).toBe('SDXL');
  expect(row.secondary).toBe('v2 · pack');
  // The full title stays available on hover.
  expect(row.tooltip.split('\n')[0]).toBe('SDXL/Text to Image');
});

test('a name that only looks like another model route is kept whole', () => {
  const row = cubeStackCartRow('key', { target_model: 'Anima' }, 'Other/Text to Image', false);
  expect(row.label).toBe('Other/Text to Image');
  expect(row.targetModel).toBe('Anima');
});
