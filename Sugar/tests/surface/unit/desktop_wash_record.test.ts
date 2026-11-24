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
/** The page and the native canvas paint the same desktop washes, from one shared record. */
import { readFileSync } from 'node:fs';
import { expect, test } from '@jest/globals';
import { desktopMaterialWash } from '../../../frontend/comfyui/ui/surface/frame/DesktopMaterialWash.js';

type Wash = readonly [number, number, number, number];
const record = JSON.parse(
  readFileSync(
    new URL('../../../integrations/contracts/desktop_wash_vectors.json', import.meta.url),
    'utf8',
  ),
) as Record<'body' | 'chrome' | 'card' | 'rowHover' | 'rowPressed', Record<'dark' | 'light', Wash>>;

const css = ([red, green, blue, alpha]: Wash): string =>
  `rgba(${String(red)}, ${String(green)}, ${String(blue)}, ${(alpha / 255).toFixed(4)})`;

test('every desktop wash the page paints is the one the shared record names', () => {
  for (const [theme, dark] of [
    ['dark', true],
    ['light', false],
  ] as const) {
    const wash = desktopMaterialWash(dark);
    expect(wash.body).toBe(css(record.body[theme]));
    expect(wash.chrome).toBe(css(record.chrome[theme]));
    expect(wash.card).toBe(css(record.card[theme]));
    expect(wash.rowHover).toBe(css(record.rowHover[theme]));
    expect(wash.rowPressed).toBe(css(record.rowPressed[theme]));
  }
});
