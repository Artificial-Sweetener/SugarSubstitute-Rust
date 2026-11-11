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
/** Verify declared model identity survives arbitrary instance title changes. */

import { resolveCubeModelTitle } from '../../../frontend/comfyui/ui/cube/CubeModelTitlePresentation.js';

test('segments only the declared model route and composes its version suffix', () => {
  expect(
    resolveCubeModelTitle({
      targetModel: 'Anima',
      title: 'Anima/Prompt by Region',
      suffix: 'version 3.2.0',
    }),
  ).toEqual({
    accessibleText: 'Anima/Prompt by Region version 3.2.0',
    modelText: 'Anima',
    nameText: 'Prompt by Region',
    suffixText: 'version 3.2.0',
    usesModelPill: true,
  });
});

test('uses declared model identity and preserves nonmatching route text', () => {
  expect(resolveCubeModelTitle({ targetModel: 'SDXL', title: 'Flux/Detailer' })).toMatchObject({
    modelText: 'SDXL',
    nameText: 'Flux/Detailer',
    accessibleText: 'SDXL Flux/Detailer',
    usesModelPill: true,
  });
  expect(resolveCubeModelTitle({ targetModel: '', title: 'Anima/Detailer' })).toMatchObject({
    modelText: '',
    nameText: 'Anima/Detailer',
    usesModelPill: false,
  });
});

test('retains the model pill for default and arbitrary instance names', () => {
  expect(
    resolveCubeModelTitle({
      targetModel: 'Anima',
      title: 'Anima/Prompt by Region',
    }).usesModelPill,
  ).toBe(true);
  expect(
    resolveCubeModelTitle({
      targetModel: 'Anima',
      title: 'Background generator',
    }),
  ).toMatchObject({ modelText: 'Anima', nameText: 'Background generator', usesModelPill: true });
});

test.each([
  ['Anima/Contextual Upscale 2', 'Contextual Upscale 2'],
  ['Final polish', 'Final polish'],
  ['Flux/Final polish 42', 'Flux/Final polish 42'],
  ['<img src=x>', '<img src=x>'],
])('keeps declared model identity for renamed instance %s', (instanceTitle, nameText) => {
  expect(
    resolveCubeModelTitle({
      targetModel: 'Anima',
      title: instanceTitle,
    }),
  ).toMatchObject({ modelText: 'Anima', nameText, usesModelPill: true });
});
