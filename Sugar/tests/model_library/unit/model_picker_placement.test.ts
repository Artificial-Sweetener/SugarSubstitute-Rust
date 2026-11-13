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
/** Prove attached placement follows the production reference across constrained boundaries. */
import { modelPickerPlacement } from '../../../frontend/comfyui/ui/model_library/ModelPickerPlacement.js';

test.each([
  {
    name: 'full below',
    boundary: [0, 0, 1920, 1080],
    anchor: [100, 50, 300, 32],
    expected: [100, 82, 560, 630, 'below'],
  },
  {
    name: 'full above',
    boundary: [0, 0, 1920, 1080],
    anchor: [1300, 800, 300, 32],
    expected: [1300, 170, 560, 630, 'above'],
  },
  {
    name: 'partial below',
    boundary: [0, 0, 700, 500],
    anchor: [650, 120, 40, 32],
    expected: [132, 152, 560, 340, 'below'],
  },
  {
    name: 'partial above',
    boundary: [0, 0, 700, 500],
    anchor: [650, 360, 40, 32],
    expected: [132, 8, 560, 352, 'above'],
  },
  {
    name: 'equal sides prefer below',
    boundary: [0, 0, 700, 500],
    anchor: [100, 234, 300, 32],
    expected: [100, 266, 560, 226, 'below'],
  },
  {
    name: 'narrow boundary',
    boundary: [0, 0, 200, 400],
    anchor: [1, 90, 198, 32],
    expected: [8, 122, 184, 270, 'below'],
  },
  {
    name: 'short boundary',
    boundary: [0, 0, 572, 100],
    anchor: [1, 40, 570, 32],
    expected: [8, 8, 556, 32, 'above'],
  },
  {
    name: 'tiny boundary drops margins',
    boundary: [0, 0, 12, 12],
    anchor: [1, 5, 10, 2],
    expected: [0, 7, 12, 5, 'below'],
  },
  {
    name: 'translated boundary',
    boundary: [100, 200, 300, 400],
    anchor: [380, 290, 18, 32],
    expected: [108, 322, 284, 270, 'below'],
  },
  {
    name: 'exact preferred height',
    boundary: [0, 0, 700, 800],
    anchor: [100, 130, 300, 32],
    expected: [100, 162, 560, 630, 'below'],
  },
  {
    name: 'exact margin width',
    boundary: [0, 0, 16, 400],
    anchor: [1, 90, 14, 32],
    expected: [0, 122, 16, 270, 'below'],
  },
  {
    name: 'zero boundary keeps measurable box',
    boundary: [0, 0, 0, 0],
    anchor: [0, 0, 0, 0],
    expected: [0, 0, 1, 1, 'below'],
  },
])('matches the reference for $name', ({ boundary, anchor, expected }) => {
  const rectangle = (values: readonly number[]) => ({
    left: values[0] ?? 0,
    top: values[1] ?? 0,
    width: values[2] ?? 0,
    height: values[3] ?? 0,
  });
  const placement = modelPickerPlacement(rectangle(boundary), rectangle(anchor));
  expect([
    placement.left,
    placement.top,
    placement.width,
    placement.height,
    placement.mode,
  ]).toEqual(expected);
});
