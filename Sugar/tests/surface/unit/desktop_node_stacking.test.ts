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
/** A node card hides the cards beneath it where it covers them, as Comfy's opaque cards do. */
import { expect, test } from '@jest/globals';
import {
  coveredParts,
  stackingMask,
  type StackedCard,
} from '../../../frontend/comfyui/ui/surface/frame/DesktopNodeStacking.js';

const card = (bounds: [number, number, number, number], order: number): StackedCard => ({
  element: document.createElement('div'),
  bounds,
  order,
});

test('a card is covered only by the cards above it, in its own units', () => {
  const bottom = card([0, 0, 400, 300], 0);
  const middle = card([300, 200, 200, 200], 1);
  const top = card([350, 250, 50, 50], 2);
  const apart = card([2000, 0, 100, 100], 3);
  const parts = coveredParts([top, apart, bottom, middle]);
  expect(parts.get(bottom)).toEqual([
    { x: 300, y: 200, width: 200, height: 200 },
    { x: 350, y: 250, width: 50, height: 50 },
  ]);
  expect(parts.get(middle)).toEqual([{ x: 50, y: 50, width: 50, height: 50 }]);
  expect(parts.get(top)).toEqual([]);
  expect(parts.get(apart)).toEqual([]);
});

test('the mask takes the covered parts out at the node opacity setting, and leaves ports beyond the box', () => {
  const mask = decodeURIComponent(
    stackingMask(400, 300, [{ x: 300, y: 200, width: 200, height: 200 }], 0.6),
  );
  // The whole card and a margin around it stay; the covered part is darkened by the setting.
  expect(mask).toContain("<rect width='464' height='364' fill='white'/>");
  expect(mask).toContain(
    "<rect x='332' y='232' width='200' height='200' rx='4' fill='black' fill-opacity='0.6'/>",
  );
});
