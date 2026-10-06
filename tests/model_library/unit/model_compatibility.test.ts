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
/** Prove compatibility and reusable queries use evidence while preserving unknown identities. */
import {
  queryModelCatalog,
  modelCompatibility,
  resolveInstalledModel,
} from '../../../frontend/comfyui/ui/model_library/ModelCompatibility.js';
import { compatibleModel } from '../support/model_compatibility.js';

test('filters by directional decisions, keeps unknown distinct and favors full installed matches', () => {
  const partial = compatibleModel('partial.safetensors', 'loras', 'SDXL', 'SDXL', [
    { family: 'Illustrious', support: 'partial' },
  ]);
  const full = compatibleModel('full.safetensors', 'loras', 'SDXL', 'Illustrious', [
    { family: 'Illustrious', support: 'full' },
  ]);
  const wrong = compatibleModel('wrong.safetensors', 'loras', 'FLUX', 'Flux1');
  const unknown = { ...wrong, traits: null };
  const context = {
    architecture: 'SDXL',
    family: 'Illustrious',
    addon: true,
    label: 'Illustrious',
  };
  expect(modelCompatibility(partial, context)).toBe('partial');
  expect(modelCompatibility(wrong, context)).toBe('incompatible');
  expect(modelCompatibility(unknown, context)).toBe('unknown');
  expect(
    modelCompatibility({ ...full, traits: { ...full.traits!, conflict: true } }, context),
  ).toBe('unknown');
  expect(queryModelCatalog([partial, full, wrong, unknown], { compatibleWith: context })).toEqual([
    partial,
    full,
  ]);
  expect(resolveInstalledModel([partial, full, wrong], 'missing', context)).toBe(full);
  expect(resolveInstalledModel([partial, full, wrong], partial.value, context)).toBe(partial);
  expect(
    queryModelCatalog([partial, full, wrong], {
      architecture: 'SDXL',
      family: 'Illustrious',
      nsfw: 'sfw',
      kind: 'loras',
      text: 'full',
    }),
  ).toEqual([full]);
});
