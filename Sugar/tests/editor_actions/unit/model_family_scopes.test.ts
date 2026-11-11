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
/** Characterize Substitute's family evidence, ordering and stable CivitAI association keys. */
import { expect, test } from '@jest/globals';
import { modelFamilyScopes } from '../../../frontend/comfyui/ui/editor_actions/domain/ModelFamilyScopes.js';
test('prefers the base model and inspects title/tags only for missing or broad SDXL evidence', () => {
  expect(modelFamilyScopes({ baseModel: 'SDXL 1.0', title: 'NoobAI XL', tags: ['Pony'] })).toEqual([
    { scope: 'model_family', provider: 'civitai', key: 'sdxl', label: 'SDXL' },
    { scope: 'model_family', provider: 'civitai', key: 'noobai', label: 'NoobAI' },
    { scope: 'model_family', provider: 'civitai', key: 'pony', label: 'Pony' },
  ]);
  expect(modelFamilyScopes({ baseModel: 'Illustrious', title: 'Flux', tags: [] })).toEqual([
    { scope: 'model_family', provider: 'civitai', key: 'illustrious', label: 'Illustrious' },
  ]);
  expect(modelFamilyScopes(null)).toEqual([]);
});

test('keeps unrecognized catalog metadata global while offering known base-model scopes', () => {
  expect(modelFamilyScopes({ baseModel: 'Other', title: 'illustriousXL_v01', tags: [] })).toEqual(
    [],
  );
  expect(modelFamilyScopes({ baseModel: 'SDXL 1.0', title: 'Animagine XL 4.0', tags: [] })).toEqual(
    [{ scope: 'model_family', provider: 'civitai', key: 'sdxl', label: 'SDXL' }],
  );
});

test('offers detected SDXL and provider family scopes independently with stable preset keys', () => {
  expect(
    modelFamilyScopes({
      architecture: 'SDXL',
      baseModel: 'Illustrious',
      title: 'Flux',
      tags: [],
    }).map((scope) => scope.key),
  ).toEqual(['sdxl', 'illustrious']);
  expect(
    modelFamilyScopes({ architecture: 'SDXL', baseModel: 'Other', title: '', tags: [] }).map(
      (scope) => scope.key,
    ),
  ).toEqual(['sdxl']);
  expect(
    modelFamilyScopes({ architecture: 'SDXL', baseModel: 'SDXL 1.0', title: '', tags: [] }).map(
      (scope) => scope.key,
    ),
  ).toEqual(['sdxl']);
});

test.each(['SD1.5', 'FLUX.2', 'Krea2', 'WAN21_I2V', 'WAN22_T2V', 'FutureComfyModel'])(
  'offers a scope for every detected architecture: %s',
  (architecture) => {
    expect(modelFamilyScopes({ architecture, baseModel: '', title: '', tags: [] })).toEqual([
      {
        scope: 'model_family',
        provider: 'comfy',
        key: architecture.toLowerCase(),
        label: architecture,
      },
    ]);
  },
);
