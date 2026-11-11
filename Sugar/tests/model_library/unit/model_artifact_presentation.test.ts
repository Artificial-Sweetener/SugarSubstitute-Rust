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
/** Prove small artifacts remain distinguishable and large weights use binary size units. */
import { modelArtifactSize } from '../../../frontend/comfyui/ui/model_library/ModelArtifactPresentation.js';

test.each([
  [0, '0 B'],
  [10, '10 B'],
  [8448, '8.25 KiB'],
  [1024 ** 2, '1 MiB'],
  [6938350040, '6.46 GiB'],
])('formats %i bytes as %s', (bytes, expected) => {
  expect(modelArtifactSize(bytes)).toBe(expected);
});
