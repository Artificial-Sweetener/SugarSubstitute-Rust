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
/** Independent physical-fragment specimens establish precise hotspot capture without browser-layout claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import type { ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
const fragments: readonly ReorderRect[] = [
  { kind: 'chip', index: 4, left: 80, right: 145, top: 20, bottom: 40 },
  { kind: 'chip', index: 4, left: 10, right: 100, top: 40, bottom: 60 },
  { kind: 'chip', index: 5, left: 160, right: 190, top: 20, bottom: 40 },
];
test('wrapped held rich fragments retain union footprint and the noncentral pressed fragment', () => {
  const grab = reorderGrabGeometry(fragments, 4, { x: 92, y: 54 });
  assert.ok(grab);
  assert.deepEqual(
    [grab.left, grab.top, grab.width, grab.height, grab.grabX, grab.grabY, grab.pressedFragment],
    [10, 20, 135, 40, 82, 34, 1],
  );
  assert.equal(grab.fragments.length, 2);
  assert.equal(Object.isFrozen(grab.fragments), true);
});
test('translated footprints preserve local grab coordinates without clamping to a first-line center', () => {
  const press = { x: 87, y: 25 },
    grab = reorderGrabGeometry(fragments, 4, press)!;
  const shifted = reorderGrabGeometry(
    fragments.map((rect) => ({
      ...rect,
      left: rect.left - 230,
      right: rect.right - 230,
      top: rect.top + 91,
      bottom: rect.bottom + 91,
    })),
    4,
    { x: press.x - 230, y: press.y + 91 },
  )!;
  assert.deepEqual(
    [shifted.grabX, shifted.grabY, shifted.width, shifted.height],
    [grab.grabX, grab.grabY, grab.width, grab.height],
  );
  assert.equal(reorderGrabGeometry(fragments, 4, { x: 30, y: 25 }), null);
});
