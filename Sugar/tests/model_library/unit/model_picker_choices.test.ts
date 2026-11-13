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
/** Prove model search and display enrichment never change Comfy's allowable values. */
import {
  modelChoiceFolders,
  filterModelChoices,
  modelPickerChoices,
} from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelChoiceCompletion } from '../../../frontend/comfyui/ui/model_library/ModelPickerCompletion.js';

test('preserves exact values, missing metadata, nested folders, and sentinel choices', () => {
  const values = [
    'None',
    'Anima\\alpha.safetensors',
    'Anima\\Portrait\\beta.safetensors',
    'other/gamma.gguf',
  ];
  const items = modelPickerChoices([], 'loras', values);
  expect(items.map((item) => item.value)).toEqual(values);
  expect(modelChoiceFolders(items, '')).toEqual(['Anima', 'other']);
  expect(modelChoiceFolders(items, 'Anima')).toEqual(['Anima/Portrait']);
  expect(filterModelChoices(items, '', 'Anima').map((item) => item.title)).toEqual([
    'alpha',
    'beta',
  ]);
  expect(filterModelChoices(items, 'portrait beta', '').map((item) => item.value)).toEqual([
    values[2],
  ]);
  expect(modelChoiceCompletion(items[1], 'al')?.suffix).toBe('pha');
  expect(modelChoiceCompletion(items[1], 'something else')?.text).toBe('alpha');
});

test('renders markup-like labels as unchanged text data', () => {
  const items = modelPickerChoices([], 'loras', ['<img src=x onerror=bad>.safetensors']);
  expect(items[0]?.title).toBe('<img src=x onerror=bad>');
});
