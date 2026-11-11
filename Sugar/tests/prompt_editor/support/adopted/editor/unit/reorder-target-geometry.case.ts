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
/** Actual pending keyboard and pointer targets renew across native geometry supersession without losing ordered intent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { dragPointer, pointer, key } from './reorder-session-fixture.ts';
import {
  paintFixture,
  pointerTransferFixture,
  arrowGeometryFixture,
  drain,
  geometryUpdate,
} from './reorder-marker-fixture.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth, undo, redo } from '@codemirror/commands';

for (const release of [false, true])
  for (const deferred of [false, true])
    test(`actual geometry supersedes already-read ${release ? 'release' : 'motion'} before ${deferred ? 'deferred placement' : 'write'}`, () => {
      const { f, shiftGeometry } = paintFixture();
      try {
        f.view.posAtCoords = () => 6;
        f.setChipGeometry(2, 15, 50);
        f.owner.pointerdown(pointer(1, 50, 15));
        drain(f);
        dragPointer(f.owner, pointer(1, 50, 15));
        drain(f);
        const selection = f.state.selection;
        if (release) f.owner.pointerup(pointer(1, 50, 55));
        else f.owner.pointermove(pointer(1, 50, 55));
        const pending = f.jobs.shift()!,
          measured = pending.read(f.view);
        assert.ok(measured && typeof measured === 'object');
        assert.deepEqual(Reflect.get(measured, 'target'), {
          kind: 'row',
          row: 0,
          insertion: 2,
        });
        assert.deepEqual(Reflect.get(measured, 'coordinates'), {
          x: 55,
          y: 50,
        });
        assert.deepEqual(Reflect.get(Reflect.get(measured, 'hidden') as object, 'order'), [0, 1]);
        assert.notEqual(
          Reflect.get(measured, 'hidden'),
          f.state.field(reorderPreview).value!.order,
        );
        if (deferred) pending.write!(measured, f.view);
        shiftGeometry(100, 0);
        f.owner.update(geometryUpdate(f, true, false));
        if (!deferred) pending.write!(measured, f.view);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        drain(f);
        if (!release) {
          assert.deepEqual(f.state.field(reorderPreview).value!.order.order, [2, 0, 1]);
          assert.equal(f.state.selection, selection);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(undoDepth(f.state), 0);
          f.owner.pointerup(pointer(1, 50, 55));
          drain(f);
        }
        assert.equal(readExactSource(f.state), 'c, a, b');
        assert.equal(undoDepth(f.state), 1);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(f.captures.size, 0);
        assert.equal(f.jobs.length + f.tasks.length + f.frames.size, 0);
      } finally {
        f.destroy();
      }
    });

for (const origin of [false, true])
  for (const deferred of [false, true])
    for (const changed of [false, true])
      for (const earlyRelease of [false, true])
        test(`actual ${origin ? 'held-origin' : 'hidden-target'} geometry renews before ${deferred ? 'deferred' : 'write'}, changed=${changed}, earlyRelease=${earlyRelease}`, () => {
          const { f, reflow, shiftOrigin } = arrowGeometryFixture();
          try {
            assert.equal(f.owner.keydown(key('Alt')), true);
            drain(f);
            const selection = f.state.selection;
            assert.equal(f.owner.keydown(key('ArrowDown')), true);
            if (!origin) {
              f.measure();
              f.task();
            }
            const pending = f.jobs.shift()!,
              measured = pending.read(f.view);
            assert.ok(measured);
            if (deferred) pending.write!(measured, f.view);
            if (changed) {
              if (origin) shiftOrigin();
              else reflow();
            }
            f.owner.update(geometryUpdate(f, changed && origin, changed && !origin));
            if (earlyRelease) assert.equal(f.owner.keyup(key('Alt')), true);
            if (!deferred) pending.write!(measured, f.view);
            assert.equal(readExactSource(f.state), 'a, b, c');
            assert.equal(f.state.selection, selection);
            assert.equal(undoDepth(f.state), 0);
            drain(f);
            const expected = changed ? (origin ? 'b, a, c' : 'b, c, a') : 'b, a, c';
            if (!earlyRelease) {
              assert.deepEqual(
                f.state.field(reorderPreview).value!.order.order,
                changed ? (origin ? [1, 0, 2] : [1, 2, 0]) : [1, 0, 2],
              );
              assert.equal(f.state.selection, selection);
              assert.equal(f.owner.keyup(key('Alt')), true);
              drain(f);
            }
            assert.equal(readExactSource(f.state), expected);
            assert.equal(undoDepth(f.state), 1);
            assert.equal(f.state.field(reorderPreview).value, null);
            assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
            {
              assert.equal(undo(f.view), true);
              assert.equal(readExactSource(f.state), 'a, b, c');
              assert.equal(redo(f.view), true);
              assert.equal(readExactSource(f.state), expected);
            }
          } finally {
            f.destroy();
          }
        });

test('geometry renewal preserves a queued native arrow before early Alt release', () => {
  const { f, reflow } = arrowGeometryFixture();
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.keydown(key('ArrowDown'));
    f.measure();
    f.task();
    const pending = f.jobs.shift()!,
      measured = pending.read(f.view);
    f.owner.keydown(key('ArrowLeft'));
    f.owner.keyup(key('Alt'));
    reflow();
    f.owner.update(geometryUpdate(f, true, false));
    pending.write!(measured, f.view);
    drain(f);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.jobs.length + f.tasks.length, 0);
  } finally {
    f.destroy();
  }
});

for (const destroyed of [false, true])
  test(`renewed arrow cannot revive resources after ${destroyed ? 'destroy' : 'Escape'}`, () => {
    const { f, reflow } = arrowGeometryFixture();
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      f.owner.keydown(key('ArrowDown'));
      f.measure();
      f.task();
      const pending = f.jobs.shift()!,
        measured = pending.read(f.view);
      reflow();
      f.owner.update(geometryUpdate(f, true, false));
      if (destroyed) f.owner.destroy();
      else f.owner.keydown(key('Escape'));
      pending.write!(measured, f.view);
      drain(f);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

test('renewed hidden-layout publication failure keeps its observable cause and retires pending intent', () => {
  const { f, reflow } = arrowGeometryFixture();
  const cause = new Error('actual-renewal-observer');
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.keydown(key('ArrowDown'));
    f.measure();
    f.task();
    const pending = f.jobs.shift()!,
      measured = pending.read(f.view);
    f.observe(() => {
      f.observe(null);
      throw cause;
    });
    reflow();
    f.owner.update(geometryUpdate(f, true, false));
    pending.write!(measured, f.view);
    assert.throws(
      () => drain(f),
      (error: unknown) => error === cause,
    );
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});
for (const changed of [false, true])
  for (const deferred of [false, true])
    for (const earlyRelease of [false, true])
      test(`pending keyboard transfers to genuine pointer hold, geometry=${changed}, deferred=${deferred}, released=${earlyRelease}`, () => {
        const transport = pointerTransferFixture(),
          { f, shiftGeometry } = transport;
        try {
          f.owner.keydown(key('Alt'));
          drain(f);
          f.owner.keydown(key('ArrowRight'));
          f.measure();
          f.task();
          const pending = f.jobs.shift()!,
            measured = pending.read(f.view),
            selection = f.state.selection;
          assert.ok(measured);
          if (deferred) pending.write!(measured, f.view);
          f.owner.keydown(key('ArrowLeft'));
          if (earlyRelease) {
            f.owner.keyup(key('Alt'));
            f.owner.keydown(key('Alt'));
          }
          assert.equal(f.owner.pointerdown(transport.down(1)), true);
          assert.equal(f.captures.has(9), true);
          drain(f);
          dragPointer(f.owner, transport.down(1));
          if (changed) {
            shiftGeometry(100, 0);
            f.owner.update(geometryUpdate(f, true, false));
          }
          if (!deferred) pending.write!(measured, f.view);
          drain(f);
          assert.equal(f.state.selection, selection);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(undoDepth(f.state), 0);
          assert.equal(f.owner.blocksInput(), true);
          assert.equal(f.captures.size, 1);
          assert.equal(f.owner.pointerup(pointer(9, 50, 55)), true);
          drain(f);
          const expected = changed ? 'b, a, c' : 'a, c, b';
          assert.equal(readExactSource(f.state), expected);
          assert.equal(undoDepth(f.state), 1);
          assert.equal(f.state.field(reorderPreview).value, null);
          assert.equal(f.owner.blocksInput(), false);
          assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
          assert.equal(undo(f.view), true);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(redo(f.view), true);
          assert.equal(readExactSource(f.state), expected);
        } finally {
          transport.destroy();
        }
      });

for (const lost of [false, true])
  test(`pointer authority delegates a later genuine keyboard session after ${lost ? 'capture loss' : 'Alt release'}`, () => {
    const { f } = paintFixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      dragPointer(f.owner, pointer());
      drain(f);
      const publication = f.state.field(reorderPreview).value;
      assert.equal(f.owner.keydown(key('ArrowRight')), true);
      drain(f);
      assert.equal(f.state.field(reorderPreview).value, publication);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      if (lost) f.owner.pointerLost(pointer());
      else f.owner.keyup(key('Alt'));
      drain(f);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.captures.size + f.frames.size, 0);
      f.owner.keydown(key('Alt'));
      drain(f);
      f.owner.keydown(key('ArrowRight'));
      drain(f);
      f.owner.keyup(key('Alt'));
      drain(f);
      assert.equal(readExactSource(f.state), 'b, a, c');
      assert.equal(undoDepth(f.state), 1);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
    } finally {
      f.destroy();
    }
  });

test('a new primary hold releases the old capture and target before changing chip identity', () => {
  const transport = pointerTransferFixture(),
    { f, nodes, shiftGeometry } = transport;
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    f.owner.pointerdown(transport.down(1));
    drain(f);
    dragPointer(f.owner, transport.down(1));
    drain(f);
    f.owner.pointermove(transport.down(1));
    const pending = f.jobs.shift()!,
      measured = pending.read(f.view);
    assert.ok(measured && typeof measured === 'object');
    assert.deepEqual(Reflect.get(measured, 'target'), {
      kind: 'row',
      row: 0,
      insertion: 2,
    });
    assert.deepEqual(Reflect.get(measured, 'coordinates'), { x: 55, y: 50 });
    assert.deepEqual(Reflect.get(Reflect.get(measured, 'hidden') as object, 'order'), [0, 2]);
    assert.notEqual(Reflect.get(measured, 'hidden'), f.state.field(reorderPreview).value!.order);
    assert.equal(f.owner.pointerdown(transport.down(2, 10)), true);
    assert.equal(f.captures.has(9), false);
    assert.equal(f.captures.has(10), true);
    drain(f);
    dragPointer(f.owner, transport.down(2, 10));
    assert.equal(nodes.filter((n) => n.className === 'sp-reorder-proxy' && !n.removed).length, 1);
    shiftGeometry(100, 0);
    f.owner.update(geometryUpdate(f, true, false));
    pending.write!(measured, f.view);
    drain(f);
    assert.equal(f.owner.pointerLost(pointer(9)), false);
    assert.equal(f.owner.pointerup(pointer(9)), false);
    assert.equal(f.owner.blocksInput(), true);
    assert.equal(f.owner.pointerup(pointer(10, 50, 55)), true);
    drain(f);
    assert.equal(readExactSource(f.state), 'c, a, b');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    transport.destroy();
  }
});

test('transfer cleanup failure preserves its cause and retires the actual session', () => {
  const transport = pointerTransferFixture(),
    { f } = transport;
  const cause = new Error('actual-transfer-release');
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    const release = f.view.contentDOM.releasePointerCapture;
    f.view.contentDOM.releasePointerCapture = (id) => {
      release.call(f.view.contentDOM, id);
      throw cause;
    };
    assert.throws(
      () => f.owner.pointerdown(transport.down(1)),
      (error: unknown) => error === cause,
    );
    drain(f);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
  } finally {
    transport.destroy();
  }
});

for (const reentrant of [false, true])
  for (const failure of [false, true])
    test(`native capture-release continuation respects the latest hold: reentrant=${reentrant}, failure=${failure}`, () => {
      const transport = pointerTransferFixture(),
        { f } = transport,
        cause = new Error('retarget-release-boundary');
      try {
        f.owner.keydown(key('Alt'));
        drain(f);
        f.owner.pointerdown(transport.down(1, 9));
        drain(f);
        dragPointer(f.owner, transport.down(1, 9));
        drain(f);
        const release = f.view.contentDOM.releasePointerCapture;
        let nested = false;
        f.view.contentDOM.releasePointerCapture = (id) => {
          release.call(f.view.contentDOM, id);
          if (id === 9) {
            if (reentrant && !nested) {
              nested = true;
              assert.equal(f.owner.pointerdown(transport.down(2, 11)), true);
            }
            if (failure) throw cause;
          }
        };
        if (failure)
          assert.throws(
            () => f.owner.pointerdown(transport.down(0, 10)),
            (error: unknown) => error === cause,
          );
        else assert.equal(f.owner.pointerdown(transport.down(0, 10)), true);
        drain(f);
        assert.equal(nested, reentrant);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(undoDepth(f.state), 0);
        if (failure && !reentrant) {
          assert.equal(f.owner.blocksInput(), false);
          assert.equal(f.state.field(reorderPreview).value, null);
          assert.equal(f.owner.pointerup(pointer(10, 50, 55)), false);
        } else {
          if (reentrant) {
            transport.shiftGeometry(100, 0);
            f.owner.update(geometryUpdate(f, true, false));
            drain(f);
          }
          const accepted = reentrant ? 11 : 10,
            rejected = reentrant ? 10 : 11;
          assert.deepEqual([...f.captures], [accepted]);
          dragPointer(f.owner, transport.down(reentrant ? 2 : 0, accepted));
          assert.equal(f.owner.pointerLost(pointer(9)), false);
          assert.equal(f.owner.pointerup(pointer(rejected, 50, 55)), false);
          assert.equal(f.owner.blocksInput(), true);
          assert.equal(f.owner.pointerup(pointer(accepted, 50, 55)), true);
          drain(f);
          const expected = reentrant ? 'c, a, b' : 'c, b, a';
          assert.equal(readExactSource(f.state), expected);
          assert.equal(undoDepth(f.state), 1);
          assert.equal(f.owner.blocksInput(), false);
          assert.equal(f.state.field(reorderPreview).value, null);
          assert.equal(undo(f.view), true);
          assert.equal(readExactSource(f.state), 'a, b, c');
          assert.equal(redo(f.view), true);
          assert.equal(readExactSource(f.state), expected);
        }
        assert.equal(f.jobs.length + f.tasks.length + f.captures.size + f.frames.size, 0);
      } finally {
        transport.destroy();
      }
    });
