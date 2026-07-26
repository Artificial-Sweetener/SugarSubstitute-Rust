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
/** Verify Cube input boundaries retain exact internal socket types. */

import { resolveCubeInputBoundaryType } from '../../frontend/comfyui/ui/cube/CubeBoundaryTypeResolver.js';

describe('resolveCubeInputBoundaryType', () => {
  test('retains one exact type shared by every internal target', () => {
    expect(resolveCubeInputBoundaryType([{ type: 'IMAGE' }, { type: 'IMAGE' }])).toBe('IMAGE');
  });

  test('uses wildcard when target types disagree or are absent', () => {
    expect(resolveCubeInputBoundaryType([{ type: 'IMAGE' }, { type: 'MASK' }])).toBe('*');
    expect(resolveCubeInputBoundaryType([{ type: null }])).toBe('*');
  });
});
