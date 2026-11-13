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
/** Prove highlighted completion channels and safe literal suffixes from Substitute's policy. */
import { expect, test } from '@jest/globals';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelChoiceCompletion } from '../../../frontend/comfyui/ui/model_library/ModelPickerCompletion.js';

test('uses the selected candidate and honors filename, path, substring, and friendly-name channels', () => {
  const item = modelPickerChoices([], 'checkpoints', ['Anima\\model_alpha.safetensors'])[0];
  if (!item) throw new Error('Missing completion fixture');
  const friendly = { ...item, title: 'Anima Model', version: 'Preview 3' };
  expect(modelChoiceCompletion(friendly, 'model_')).toEqual({
    suffix: 'alpha',
    text: 'model_alpha',
  });
  expect(modelChoiceCompletion(friendly, 'Anima\\mod')?.text).toBe('Anima\\model_alpha');
  expect(modelChoiceCompletion(friendly, 'Anima/mod')?.text).toBe('Anima/model_alpha');
  expect(modelChoiceCompletion(friendly, 'alpha')).toBeNull();
  expect(modelChoiceCompletion(friendly, 'alp')?.suffix).toBe('ha');
  expect(modelChoiceCompletion(friendly, 'lp')?.suffix).toBe('ha');
  expect(modelChoiceCompletion(friendly, 'Anima M')?.suffix).toBe('odel - Preview 3');
  expect(modelChoiceCompletion(friendly, '')).toEqual({
    suffix: 'model_alpha',
    text: 'model_alpha',
  });
  expect(modelChoiceCompletion(undefined, 'model_')).toBeNull();
  expect(modelChoiceCompletion(friendly, 'no match')).toEqual({
    suffix: ' → model_alpha',
    text: 'model_alpha',
  });
});
