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
/** Actual finite native owners prove donor press admission and Manhattan threshold without claiming browser geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { fixture, key, pointer } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';

for (const [dx, dy, starts] of [
  [0, 0, false],
  [9, 0, false],
  [4, 5, false],
  [10, 0, true],
  [4, 6, true],
  [-5, -5, true],
] as const)
  test(`actual primary press waits for donor Manhattan threshold dx=${dx}, dy=${dy}`, () => {
    const f = fixture();
    try {
      const selection = f.state.selection;
      f.owner.pointerdown(pointer(1, 20, 15));
      drain(f);
      const preview = f.state.field(reorderPreview).value;
      assert.ok(preview);
      assert.equal(preview.order.order.length, 3);
      assert.equal(f.captures.size, 1);
      assert.equal(f.frames.size, 0);
      assert.equal(f.removed, 0);
      f.owner.pointermove(pointer(1, 20 + dy, 15 + dx));
      // Threshold reserves admission; its genuine READ and deferred WRITE publish the held preview.
      assert.equal(f.state.field(reorderPreview).value, preview);
      if (starts) {
        f.measure();
        f.task();
      }
      assert.equal(f.state.field(reorderPreview).value === preview, !starts);
      assert.equal(f.frames.size, starts ? 1 : 0);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      f.owner.keydown(key('Escape'));
      assert.equal(f.captures.size + f.frames.size, 0);
      assert.equal(f.removed, starts ? 1 : 0);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

for (const end of ['up', 'alt', 'escape', 'destroy', 'source'] as const)
  test(`unchanged press ${end} retires capture without a proxy or history`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer());
      f.measure();
      f.task();
      f.owner.pointermove(pointer(1, 24, 19));
      if (end === 'up') assert.equal(f.owner.pointerup(pointer()), true);
      else if (end === 'alt') f.owner.keyup(key('Alt'));
      else if (end === 'escape') f.owner.keydown(key('Escape'));
      else if (end === 'destroy') f.owner.destroy();
      else f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      if (end === 'destroy' || end === 'source') {
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.captures.size, 1);
        f.task();
      }
      assert.equal(f.captures.size + f.frames.size, 0);
      assert.equal(f.removed, 0);
      assert.equal(readExactSource(f.state), end === 'source' ? 'xa, b, c' : 'a, b, c');
      assert.equal(undoDepth(f.state), end === 'source' ? 1 : 0);
      if (end === 'up') {
        assert.ok(f.state.field(reorderPreview).value);
        assert.equal(f.owner.keydown(key('ArrowRight')), true);
        drain(f);
        f.owner.keyup(key('Alt'));
        drain(f);
        assert.equal(readExactSource(f.state), 'b, a, c');
        assert.equal(undoDepth(f.state), 1);
      }
    } finally {
      f.destroy();
    }
  });

test('motion before entry measurement retains original press and latest coordinates without starting HTML drag', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.owner.pointerdown(pointer());
    assert.equal(f.owner.pointermove(pointer(1, 24, 21)), true);
    assert.equal(f.owner.pointermove(pointer(9, 80, 80)), false);
    assert.equal(f.captures.size, 0);
    f.measure();
    f.task();
    assert.equal(f.captures.size, 1);
    assert.equal(f.frames.size, 0);
    f.measure();
    f.task();
    f.measure();
    f.task();
    assert.equal(f.frames.size, 1);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    f.owner.keydown(key('Escape'));
    assert.equal(f.captures.size + f.frames.size, 0);
    assert.equal(f.removed, 1);
  } finally {
    f.destroy();
  }
});

for (const lost of [1, 9])
  test(`pending pointer capture loss ${lost} retires only the matching entry and fences late callbacks`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer());
      assert.equal(f.owner.blocksInput(), true);
      assert.equal(f.owner.pointerLost(pointer(lost)), false);
      assert.equal(f.owner.blocksInput(), lost !== 1);
      f.measure();
      if (f.tasks.length) f.task();
      assert.equal(f.captures.size, lost === 1 ? 0 : 1);
      assert.equal(f.state.field(reorderPreview).value !== null, lost !== 1);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });
