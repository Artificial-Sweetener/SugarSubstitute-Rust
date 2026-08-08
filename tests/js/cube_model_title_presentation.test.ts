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
/** Verify authoritative target-model segmentation and default-alias policy. */

import {
  resolveCubeModelTitle,
  resolveDefaultInstanceModelTitle,
} from '../../frontend/comfyui/ui/cube/CubeModelTitlePresentation.js';

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

test('does not infer a model from a nonmatching or missing declaration', () => {
  expect(resolveCubeModelTitle({ targetModel: 'SDXL', title: 'Flux/Detailer' })).toMatchObject({
    modelText: '',
    nameText: 'Flux/Detailer',
    usesModelPill: false,
  });
  expect(resolveCubeModelTitle({ targetModel: '', title: 'Anima/Detailer' })).toMatchObject({
    modelText: '',
    nameText: 'Anima/Detailer',
    usesModelPill: false,
  });
});

test('uses the model pill only for an instance still showing its default alias', () => {
  expect(
    resolveDefaultInstanceModelTitle({
      targetModel: 'Anima',
      instanceTitle: 'Anima/Prompt by Region',
      defaultAlias: 'Anima/Prompt by Region',
    }).usesModelPill,
  ).toBe(true);
  expect(
    resolveDefaultInstanceModelTitle({
      targetModel: 'Anima',
      instanceTitle: 'Background generator',
      defaultAlias: 'Anima/Prompt by Region',
    }),
  ).toMatchObject({ nameText: 'Background generator', usesModelPill: false });
});
