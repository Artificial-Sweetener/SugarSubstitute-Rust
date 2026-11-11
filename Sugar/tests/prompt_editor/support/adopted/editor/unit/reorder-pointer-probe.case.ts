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
/** Actual feedback retains the one admitted grab geometry for both positioning and donor center probes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderPointerFeedback } from '../../../../../../frontend/sugarprompt/editor/src/reorder-pointer-feedback.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import { paintFixture } from './reorder-marker-fixture.ts';
for (const press of [
  { x: 14, y: 12 },
  { x: 26, y: 28 },
])
  test(`noncentral press ${press.x}/${press.y} translates the same admitted held center`, () => {
    const f = paintFixture(),
      feedback = new ReorderPointerFeedback(f.view);
    try {
      assert.equal(feedback.probe(press), null);
      feedback.measureAppearance();
      const geometry = reorderGrabGeometry(
        [{ kind: 'chip', index: 0, left: 10, right: 30, top: 10, bottom: 30 }],
        0,
        press,
      )!;
      feedback.begin({ node: f.view.dom.ownerDocument.createElement('span'), geometry }, press);
      assert.deepEqual(feedback.probe(press), { x: 20, y: 20 });
      assert.deepEqual(feedback.probe({ x: press.x + 91, y: press.y - 37 }), {
        x: 111,
        y: -17,
      });
      feedback.close();
      assert.equal(feedback.probe(press), null);
    } finally {
      feedback.close();
      f.destroy();
    }
  });
