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
/** Verify the shared Cube-face spacing policy used by every node renderer. */

import { resolveCubeSurfaceCardSpacing } from '../../../frontend/comfyui/ui/surface/CubeSurfaceSpacing.js';

describe('resolveCubeSurfaceCardSpacing', () => {
  test('uses one gap for masonry cards and both vertical edges', () => {
    expect(resolveCubeSurfaceCardSpacing({ gap: 19 })).toEqual({
      gap: 19,
      headerInset: 19,
      footerInset: 19,
    });
  });

  test('keeps invalid runtime geometry finite', () => {
    expect(resolveCubeSurfaceCardSpacing({ gap: Number.NaN })).toEqual({
      gap: 0,
      headerInset: 0,
      footerInset: 0,
    });
    expect(resolveCubeSurfaceCardSpacing({ gap: -4 })).toEqual({
      gap: 0,
      headerInset: 0,
      footerInset: 0,
    });
  });
});
