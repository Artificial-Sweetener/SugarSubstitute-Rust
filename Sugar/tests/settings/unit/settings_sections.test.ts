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
/** Keep settings organization host-neutral and shipped licensing synchronized. */
import { expect, test } from '@jest/globals';
import { readFileSync } from 'node:fs';
import {
  sugarCubesSettingsSection,
  sugarCubesSettingsSections,
} from '../../../frontend/comfyui/ui/settings/SugarCubesSettingsSections.js';

test('project ordered sections into native Comfy tokens while preserving per-setting identity keys', () => {
  const appearance = sugarCubesSettingsSection('appearance', 'Fluent');
  const models = sugarCubesSettingsSection('models', 'EnhancedPicker');
  expect(appearance.category).toEqual(['SugarCubes', 'Appearance', 'Fluent']);
  expect(models.category).toEqual(['SugarCubes', 'Model library', 'EnhancedPicker']);
  expect(appearance.sortOrder).toBeGreaterThan(models.sortOrder);
  expect(
    new Set(Object.values(sugarCubesSettingsSections).map((section) => section.label)).size,
  ).toBe(Object.keys(sugarCubesSettingsSections).length);
});

test('ship the repository license in the offline About resource', () => {
  expect(
    readFileSync(new URL('../../../frontend/static/legal/AGPL-3.0.txt', import.meta.url), 'utf8'),
  ).toBe(readFileSync(new URL('../../../LICENSE', import.meta.url), 'utf8'));
});
