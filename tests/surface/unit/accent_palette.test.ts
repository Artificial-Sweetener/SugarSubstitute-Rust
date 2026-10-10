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
/** Prove accent validation, brand ramp ordering, and foreground contrast decisions. */
import {
  normalizeAccentColor,
  createAccentBrandPalette,
  accentForeground,
  accentIndicatorColor,
} from '../../../frontend/comfyui/ui/surface/appearance/AccentPalette.js';

test.each([
  ['#B58CFF', '#b58cff'],
  [' #abc ', '#aabbcc'],
  ['#123456', '#123456'],
])('normalizes %s', (value, expected) => {
  expect(normalizeAccentColor(value)).toBe(expected);
});

test('preserves a legible accent and adjusts extreme accents for their neutral surface', () => {
  expect(accentIndicatorColor('#479ef5', true)).toBe('#479ef5');
  expect(accentIndicatorColor('#071530', true)).not.toBe('#071530');
  expect(accentIndicatorColor('#ffe066', false)).not.toBe('#ffe066');
  expect(accentIndicatorColor('#071530', false)).toBe('#071530');
  expect(() => accentIndicatorColor('invalid', true)).toThrow(TypeError);
});
test.each(['red', 'url(x)', '#abcd', '#12345678', ';color:red', null, 42])(
  'rejects invalid accent %p',
  (value) => {
    expect(normalizeAccentColor(value)).toBeNull();
  },
);
test.each([
  ['#ffff00', '#000000'],
  ['#ffffff', '#000000'],
  ['#000000', '#ffffff'],
  ['#102060', '#ffffff'],
])('keeps text readable on %s', (color, expected) => {
  expect(accentForeground(color)).toBe(expected);
});
test('keeps the seed and every brand slot while producing distinct state colors', () => {
  const ramp = createAccentBrandPalette('#b58cff');
  expect(ramp[80]).toBe('#b58cff');
  expect(Object.keys(ramp)).toHaveLength(16);
  expect(new Set(Object.values(ramp)).size).toBe(16);
  expect(Object.values(ramp).every((color) => normalizeAccentColor(color) === color)).toBe(true);
  expect(() => createAccentBrandPalette('url(x)')).toThrow(TypeError);
});
