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
/** Compare all variants and control-role assignments with shared Qt-generated fixtures. */
import { readFileSync } from 'node:fs';
import {
  accentVariants,
  createAccentPalette,
  normalizeAccentColor,
} from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

const vectors = readFileSync(new URL('../support/accent-vectors.txt', import.meta.url), 'utf8')
  .split('\n')
  .filter((line) => line && !line.startsWith('#'))
  .map((line) => line.split(' '));

test('the shared fixture covers thirty seeds in both modes', () => {
  expect(vectors).toHaveLength(60);
});

test.each(vectors)('matches every qfluentwidgets shade for %s %s', (base, mode, ...expected) => {
  const dark = mode === 'dark';
  const variants = accentVariants('#' + base, dark);
  expect(Object.values(variants)).toEqual(expected.map((hex) => '#' + hex));
  const p = createAccentPalette('#' + base, dark);
  expect(p.button).toEqual({
    rest: variants.primary,
    hover: dark ? variants.dark1 : variants.light1,
    pressed: dark ? variants.dark2 : variants.light3,
  });
  expect(p.checkbox).toEqual({
    rest: variants.primary,
    hover: p.button.hover,
    pressed: dark ? variants.dark2 : variants.light2,
  });
  expect(p.toggle).toEqual({
    rest: variants.primary,
    hover: variants.light1,
    pressed: variants.light2,
  });
  expect(p.link).toEqual({
    rest: variants.primary,
    hover: variants.primary,
    pressed: dark ? variants.dark1 : variants.primary,
  });
  expect(p.indicator).toBe(variants.primary);
  expect(p.selection).toBe(dark ? variants.primary : variants.light1);
  expect(p.foreground).toBe(dark ? '#000000' : '#ffffff');
  expect(p.buttonBorder).toBe(variants.light1);
  expect(p.buttonBottomBorder).toBe(dark ? variants.light2 : variants.dark1);
});

test.each([
  ['#B58CFF', '#b58cff'],
  [' #abc ', '#aabbcc'],
  ['#123456', '#123456'],
])('normalizes %s', (value, expected) => {
  expect(normalizeAccentColor(value)).toBe(expected);
});

test.each(['red', 'url(x)', '#abcd', '#12345678', ';color:red', null, 42])(
  'rejects invalid accent %p',
  (value) => {
    expect(normalizeAccentColor(value)).toBeNull();
    expect(() => accentVariants(String(value), true)).toThrow(TypeError);
  },
);
