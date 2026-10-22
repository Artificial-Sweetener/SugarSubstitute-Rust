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
/** Publication-dependent native measurement transport proves keyboard settlement without certifying browser layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth, undo, redo } from '@codemirror/commands';
import { key } from './reorder-session-fixture.ts';
import {
  geometryUpdate,
  drain,
  arrowGeometryFixture,
  calibrateCollision,
} from './reorder-marker-fixture.ts';
import { publicationFixture } from './reorder-publication-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const specimen of [
  {
    source: '#Scene\na, b',
    position: 0,
    arrow: 'ArrowRight',
    expected: '\na, #Scene, b',
  },
  {
    source: 'a\n[SEP|Face]\nb',
    position: 0,
    arrow: 'ArrowRight',
    expected: '\n[SEP|Face]\nb, a',
  },
  // The narrow gap marker is an insertion anchor; its keyboard vacancy spans the viewport.
  { source: 'a\nb\nc', position: 2, arrow: 'ArrowDown', expected: 'a\n\nc, b' },
  {
    source: 'aaaaaaaaaa\nb\nc',
    position: 11,
    arrow: 'ArrowUp',
    expected: 'b, aaaaaaaaaa\n\nc',
  },
  { source: 'a, b, c', position: 3, arrow: 'ArrowLeft', expected: 'b, a, c' },
])
  for (const early of [false, true])
    test(`published layout geometry settles ${specimen.arrow} at ${specimen.position}, early release=${early}`, () => {
      const transport = publicationFixture(specimen.source, specimen.position),
        { f } = transport;
      try {
        assert.equal(f.owner.keydown(key('Alt')), true);
        transport.settle();
        const selection = f.state.selection;
        assert.equal(f.owner.keydown(key(specimen.arrow)), true);
        if (early) assert.equal(f.owner.keyup(key('Alt')), true);
        transport.settle();
        if (!early) {
          assert.equal(readExactSource(f.state), specimen.source);
          assert.equal(f.state.selection, selection);
          assert.equal(undoDepth(f.state), 0);
          assert.equal(f.owner.keyup(key('Alt')), true);
          transport.settle();
        }
        assert.ok(transport.notifications >= 2);
        assert.equal(readExactSource(f.state), specimen.expected);
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
        assert.equal(undo(f.view), true);
        assert.equal(readExactSource(f.state), specimen.source);
        assert.equal(redo(f.view), true);
        assert.equal(readExactSource(f.state), specimen.expected);
      } finally {
        f.destroy();
      }
    });

for (const retire of ['selection', 'destroy'] as const)
  test(`hidden geometry confirmation retires on ${retire}`, () => {
    const { f } = arrowGeometryFixture();
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      f.owner.keydown(key('ArrowDown'));
      f.measure();
      f.task();
      const pending = f.jobs.shift()!,
        result = pending.read(f.view);
      f.owner.update(geometryUpdate(f, true, false));
      pending.write!(result, f.view);
      calibrateCollision(f);
      const confirmation = f.jobs.shift()!;
      assert.ok(confirmation);
      const observed = confirmation.read(f.view),
        retained = f.state.field(reorderPreview).value;
      if (retire === 'selection') f.view.dispatch({ selection: EditorSelection.cursor(1) });
      else f.owner.destroy();
      const state = f.state;
      assert.equal(f.owner.blocksInput(), false);
      confirmation.write!(observed, f.view);
      assert.equal(f.state, state);
      if (retire === 'destroy') assert.equal(f.state.field(reorderPreview).value, retained);
      drain(f);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.captures.size + f.frames.size + f.jobs.length + f.tasks.length, 0);
    } finally {
      f.destroy();
    }
  });

for (const origin of [false, true])
  for (const early of [false, true])
    test(`actual ${origin ? 'origin' : 'target'} reflow before first hidden read keeps current placement, early release=${early}`, () => {
      const { f, reflow, shiftOrigin } = arrowGeometryFixture();
      try {
        f.owner.keydown(key('Alt'));
        drain(f);
        const selection = f.state.selection;
        f.owner.keydown(key('ArrowDown'));
        f.measure();
        f.task();
        if (origin) shiftOrigin();
        else reflow();
        const pending = f.jobs.shift()!,
          target = pending.read(f.view);
        assert.ok(target);
        f.owner.update(geometryUpdate(f, true, false));
        if (early) f.owner.keyup(key('Alt'));
        pending.write!(target, f.view);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(f.state.selection, selection);
        assert.equal(undoDepth(f.state), 0);
        drain(f);
        if (!early) {
          assert.deepEqual(
            f.state.field(reorderPreview).value!.order.order,
            origin ? [1, 0, 2] : [1, 2, 0],
          );
          assert.equal(f.state.selection, selection);
          f.owner.keyup(key('Alt'));
          drain(f);
        }
        assert.equal(readExactSource(f.state), origin ? 'b, a, c' : 'b, c, a');
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
      } finally {
        f.destroy();
      }
    });

for (const origin of [false, true])
  for (const deferred of [false, true])
    test(`hidden renewal rejects ${origin ? 'origin' : 'target'} reflow before ${deferred ? 'deferred' : 'write'} publication`, () => {
      const { f, reflow, shiftOrigin } = arrowGeometryFixture();
      try {
        f.owner.keydown(key('Alt'));
        drain(f);
        f.owner.keydown(key('ArrowDown'));
        f.measure();
        f.task();
        const hidden = f.jobs.shift()!,
          target = hidden.read(f.view);
        f.owner.update(geometryUpdate(f, true, false));
        hidden.write!(target, f.view);
        calibrateCollision(f);
        const confirmation = f.jobs.shift()!,
          full = confirmation.read(f.view);
        assert.ok(full);
        if (deferred) confirmation.write!(full, f.view);
        if (origin) shiftOrigin();
        else reflow();
        f.owner.update(geometryUpdate(f, true, false));
        f.owner.keyup(key('Alt'));
        if (!deferred) confirmation.write!(full, f.view);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        drain(f);
        assert.equal(readExactSource(f.state), origin ? 'b, a, c' : 'b, c, a');
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
      } finally {
        f.destroy();
      }
    });

test('viewport supersession during hidden renewal remeasures and commits once', () => {
  const { f } = arrowGeometryFixture();
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.keydown(key('ArrowDown'));
    f.measure();
    f.task();
    const hidden = f.jobs.shift()!,
      target = hidden.read(f.view);
    f.owner.update(geometryUpdate(f, true, false));
    hidden.write!(target, f.view);
    calibrateCollision(f);
    const confirmation = f.jobs.shift()!,
      full = confirmation.read(f.view);
    f.view.scrollDOM.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      left: 0,
      right: 180,
      top: 0,
      bottom: 120,
      width: 180,
      height: 120,
      toJSON: () => ({}),
    });
    f.owner.update(geometryUpdate(f, false, true));
    f.owner.keyup(key('Alt'));
    confirmation.write!(full, f.view);
    drain(f);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

test('current hidden geometry observation failure preserves cause and retires the complete keyboard lifetime', () => {
  const { f } = arrowGeometryFixture(),
    cause = new Error('controlled-geometry-failure');
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.keydown(key('ArrowDown'));
    f.measure();
    f.task();
    const hidden = f.jobs.shift()!,
      target = hidden.read(f.view);
    f.owner.update(geometryUpdate(f, true, false));
    hidden.write!(target, f.view);
    calibrateCollision(f);
    const confirmation = f.jobs.shift()!;
    f.view.scrollDOM.getBoundingClientRect = () => {
      throw cause;
    };
    assert.throws(
      () => confirmation.read(f.view),
      (actual) => actual === cause,
    );
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.frames.size + f.captures.size, 0);
  } finally {
    f.destroy();
  }
});
