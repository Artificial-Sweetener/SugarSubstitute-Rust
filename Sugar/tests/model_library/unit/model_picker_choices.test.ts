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
  orderModelsByTitle,
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

test('orders LoRAs by the name each card shows, not by file name', () => {
  const titled = (value: string, title: string) => ({
    ...requireItem(modelPickerChoices([], 'loras', [value])[0]),
    title,
  });
  const items = [
    titled('Anima/2xy_seapall.safetensors', 'Seapall - Artist Style'),
    titled('Anima/420core.safetensors', '420core'),
    titled('Anima/ag_botis.safetensors', 'AG: Botis - Polished'),
    titled('Anima/zz_bluethebone.safetensors', 'bluethebone'),
    titled('Anima/b_dup.safetensors', 'Duplicate'),
    titled('Anima/a_dup.safetensors', 'duplicate'),
    // No page metadata: the picker titles it with its file name and orders it by that.
    ...modelPickerChoices([], 'loras', ['Anima/90s_vibrant.safetensors']),
  ];
  expect(orderModelsByTitle(items).map((item) => item.title)).toEqual([
    '420core',
    '90s_vibrant',
    'AG: Botis - Polished',
    'bluethebone',
    'Duplicate',
    'duplicate',
    'Seapall - Artist Style',
  ]);
  // Equal names keep a stable order by path.
  const tied = [titled('b/x.safetensors', 'Same'), titled('a/x.safetensors', 'Same')];
  expect(orderModelsByTitle(tied).map((item) => item.value)).toEqual([
    'a/x.safetensors',
    'b/x.safetensors',
  ]);
});

test('lists folders case-insensitively', () => {
  const items = modelPickerChoices([], 'loras', [
    'zeta/a.safetensors',
    'Anima/b.safetensors',
    'beta/c.safetensors',
  ]);
  expect(modelChoiceFolders(items, '')).toEqual(['Anima', 'beta', 'zeta']);
});

/** Narrow a fixture item that must exist. */
function requireItem<T>(item: T | undefined): T {
  if (item === undefined) throw new Error('fixture item missing');
  return item;
}

test('renders markup-like labels as unchanged text data', () => {
  const items = modelPickerChoices([], 'loras', ['<img src=x onerror=bad>.safetensors']);
  expect(items[0]?.title).toBe('<img src=x onerror=bad>');
});
