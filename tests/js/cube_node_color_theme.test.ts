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
/** Verify one Cube color theme drives native cards and the darker Cube backdrop. */

import {
  deriveCubeBackdropColor,
  resolveCubeNodeColorTheme,
} from '../../frontend/comfyui/ui/surface/CubeNodeColorTheme.js';

describe('CubeNodeColorTheme', () => {
  test('uses the exact explicit native node colors for projected cards', () => {
    expect(
      resolveCubeNodeColorTheme({
        color: '#2b2859',
        bgcolor: 'rgba(32, 33, 39, 0.9)',
      }),
    ).toEqual({
      header: '#2b2859',
      body: 'rgba(32, 33, 39, 0.9)',
    });
  });

  test('does not manufacture a card theme for an uncolored Cube node', () => {
    expect(resolveCubeNodeColorTheme({ color: ' ', bgcolor: undefined })).toBeNull();
  });

  test('slightly darkens the native card body for the enclosing Cube backdrop', () => {
    expect(deriveCubeBackdropColor('rgba(32, 33, 39, 0.9)')).toBe('rgb(27 28 33 / 0.9)');
  });

  test('keeps the established safe backdrop when the host color is unsupported', () => {
    expect(deriveCubeBackdropColor('var(--host-node-color)')).toBe('#0d1117');
  });
});
