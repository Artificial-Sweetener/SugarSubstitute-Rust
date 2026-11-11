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
/** Preserve artwork shading while the shared accent owner supplies interaction shades. */
import { tintOrbPixel } from '../../../frontend/comfyui/ui/surface/frame/OrbTint.js';

test('transparent pixels, opaque accent and translucent white highlights retain their alpha', () => {
  expect(tintOrbPixel([255, 255, 255, 0], [100, 50, 25])).toEqual([0, 0, 0, 0]);
  expect(tintOrbPixel([255, 255, 255, 255], [100, 50, 25])).toEqual([100, 50, 25, 255]);
  expect(tintOrbPixel([0, 0, 0, 255], [100, 50, 25])).toEqual([58, 29, 14, 255]);
  expect(tintOrbPixel([255, 255, 255, 32], [100, 50, 25])).toEqual([255, 255, 255, 32]);
});
