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
/** Prove GIMP's upstream palette roles remain complete and distinct across light and dark variants. */
import { expect, test } from '@jest/globals';
import {
  gimpPalette,
  gimpThemeTokens,
  gimpTokenDeclarations,
} from '../../../frontend/comfyui/ui/surface/appearance/GimpThemeTokens.js';

test.each([true, false])('maps native GTK materials into every semantic role (dark=%s)', (dark) => {
  const palette = gimpPalette(dark),
    tokens = gimpThemeTokens(dark);
  expect(tokens.surface).toBe(palette.background);
  expect(tokens['field-fill']).toBe(palette.widget);
  expect(tokens['field-stroke']).toBe(palette.strongerBorder);
  expect(tokens['focus-ring']).toBe(palette.foreground);
  expect(tokens['field-focus-outline']).toContain('dotted');
  expect(tokens['control-radius']).toBe('3px');
  expect(Object.values(tokens).every((value) => value.length > 0)).toBe(true);
  expect(gimpTokenDeclarations(dark)).toContain(`--sc-gimp-trough:${palette.trough};`);
});

test('retains the exact Default theme background, foreground and nested-widget colors', () => {
  expect(gimpPalette(true)).toMatchObject({
    background: '#3c3c3c',
    foreground: '#dcdcdc',
    widget: '#2a2a2a',
    selected: '#1e1e1e',
  });
  expect(gimpPalette(false)).toMatchObject({
    background: '#ebebeb',
    foreground: '#000000',
    widget: '#dcdcdc',
    selected: '#fafafa',
  });
});
