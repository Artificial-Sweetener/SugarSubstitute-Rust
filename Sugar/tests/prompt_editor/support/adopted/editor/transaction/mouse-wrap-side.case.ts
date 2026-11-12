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
/** A click keeps the side of the soft wrap it landed on through the press and the following drag. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';

test('a single click places a collapsed caret on the side of the offset the pointer hit', () => {
  for (const side of [-1, 1] as const) {
    const native = heldTokenFixture('alpha beta gamma delta', 0);
    try {
      native.hitSide(side);
      const { event, style } = native.press(heldTokenPointer(11, 1));
      native.select(style, event);
      const main = native.state.selection.main;
      assert.deepEqual([main.anchor, main.head, main.assoc], [11, 11, side]);
    } finally {
      native.destroy();
    }
  }
});

test('dragging from a wrap-side caret keeps the pressed anchor and takes the head side from the pointer', () => {
  const native = heldTokenFixture('alpha beta gamma delta', 0);
  try {
    native.hitSide(-1);
    const { event, style } = native.press(heldTokenPointer(11, 1));
    native.select(style, event);
    native.hitSide(1);
    native.select(style, heldTokenPointer(17, 1));
    const extended = native.state.selection.main;
    assert.deepEqual([extended.anchor, extended.head], [11, 17]);
    native.select(style, heldTokenPointer(11, 1));
    const returned = native.state.selection.main;
    assert.deepEqual([returned.anchor, returned.head, returned.assoc], [11, 11, 1]);
  } finally {
    native.destroy();
  }
});
