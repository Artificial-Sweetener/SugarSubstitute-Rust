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
/** Literal native paint transport exposes only actual stylesheet and local activation marker output. */
import assert from 'node:assert/strict';
import { ReorderMotionStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-styles.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
/** The existing literal paint transport exposes actual retained node/rule identities without modeling CSS matching. */
export function motionStylesFixture() {
  const f = motionPaintFixture(),
    owner = new ReorderMotionStyles(f.view);
  return {
    f,
    owner,
    marker: () => {
      const node = f.view.dom.querySelector('[data-reorder-motion-gate]');
      assert.ok(node);
      return node as unknown as HTMLSpanElement;
    },
    sheet: () => {
      const node = f.document.head!.childNodes[0];
      assert.ok(node);
      return node as unknown as HTMLStyleElement;
    },
  };
}
