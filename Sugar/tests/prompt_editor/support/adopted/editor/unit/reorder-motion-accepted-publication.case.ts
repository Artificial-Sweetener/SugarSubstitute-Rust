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
/** Accepted geometry belongs to an immutable CM publication even while its old mounted DOM lease is still current. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { currentReorderPaintLease } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

test('new genuine publication cannot retain old accepted paint before native draw changes its mounted lease', () => {
  const f = motionPresentationFixture();
  try {
    const source = readExactSource(f.state),
      selection = f.state.selection;
    f.prepare();
    f.deliver();
    const original = f.state.field(reorderPreview).value;
    assert.ok(original);
    const oldMask = f.state.field(reorderPreview).mask,
      oldLease = currentReorderPaintLease(f.view, original),
      retained = f.presentation.retain(f.input);
    assert.ok(oldMask);
    assert.ok(oldLease);
    assert.equal(oldLease(), true);
    assert.ok(retained);
    assert.equal(retained(), true);
    assert.equal(f.jobs.length, 0);
    f.publish();
    const next = f.state.field(reorderPreview).value;
    assert.ok(next);
    assert.equal(next === original, false);
    assert.equal(next.capture === original.capture, true);
    assert.equal(next.order === original.order, true);
    assert.equal(oldLease(), true);
    assert.equal(retained(), false);
    assert.equal(f.presentation.retain(f.input), null);
    f.prepare();
    assert.equal(f.jobs.length, 1);
    f.deliver();
    assert.equal(f.jobs.length, 0);
    assert.equal(f.tasks.length, 0);
    assert.equal(f.state.field(reorderPreview).value === next, true);
    const acceptedMask = f.state.field(reorderPreview).mask;
    assert.ok(acceptedMask);
    assert.equal(acceptedMask === oldMask, false);
    assert.equal(f.presentation.hit(65, 25), 1);
    const accepted = f.presentation.retain(f.input);
    assert.ok(accepted);
    assert.equal(accepted(), true);
    const reads = f.paint.styleReads,
      allocations = f.paint.allocations;
    f.prepare();
    assert.equal(f.jobs.length, 0);
    assert.equal(f.paint.styleReads, reads);
    assert.equal(f.paint.allocations, allocations);
    assert.equal(f.state.field(reorderPreview).mask === acceptedMask, true);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection === selection, true);
    assert.equal(undoDepth(f.state), 0);
    f.presentation.cancel();
    assert.equal(accepted(), false);
    assert.equal(f.frames.size, 0);
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).mask === null, true);
    assert.equal(f.tasks.length, 0);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection === selection, true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
