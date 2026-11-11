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
/** Prove architecture evidence is validated independently of provider model-family labels. */
import { expect, test } from '@jest/globals';
import { decodeModelCatalog } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';

test('preserves additive architecture evidence without replacing the provider family', () => {
  const item = modelPickerChoices([], 'checkpoints', ['model.safetensors'])[0];
  if (!item) throw new Error('Missing model fixture');
  const response = (architecture: unknown) => ({
    schemaVersion: 1,
    revision: 1,
    fieldContracts: [],
    models: [{ ...item, baseModel: 'Illustrious', architecture }],
  });
  const architecture = { base: 'SDXL', comfyClass: 'SDXL', source: 'comfy_tensor_shapes' };
  expect(decodeModelCatalog(response(architecture)).items[0]?.architecture).toEqual(architecture);
  expect(decodeModelCatalog(response(architecture)).items[0]?.baseModel).toBe('Illustrious');
  expect(decodeModelCatalog(response(undefined)).items[0]?.architecture).toBeNull();
  expect(() => decodeModelCatalog(response({ ...architecture, source: 'filename' }))).toThrow();
  expect(() => decodeModelCatalog(response({ ...architecture, base: '' }))).toThrow();
});
