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
import { drain } from './reorder-marker-fixture.ts';
/** Actual native owner controls for RAF scheduling and reentrant entry cancellation failure frontiers. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { dragPointer, fixture, key, pointer } from './reorder-session-fixture.ts';

for (const frontier of ['initial', 'motion', 'tick'] as const)
  test(`actual ${frontier} RAF scheduling failure releases finite pointer ownership`, () => {
    const f = fixture(),
      cause = new Error('RAF scheduling failure'),
      window = f.view.dom.ownerDocument.defaultView!,
      raf = window.requestAnimationFrame;
    const selection = f.state.selection;
    try {
      if (frontier === 'initial')
        window.requestAnimationFrame = () => {
          throw cause;
        };
      f.setChipGeometry(0, 15, 119);
      f.owner.pointerdown(pointer(1, 119));
      drain(f);
      if (frontier === 'initial')
        assert.throws(
          () => {
            dragPointer(f.owner, pointer(1, 119));
            f.measure();
            f.task();
          },
          (e) => e === cause,
        );
      else {
        dragPointer(f.owner, pointer(1, 119));
        // Deliver actual cold admission before exercising the existing RAF fault boundary.
        f.measure();
        f.task();
        f.owner.pointermove(pointer(1, 119, 16));
        assert.equal(f.captures.size, 1);
        assert.equal(f.frames.size, 2);
        f.frame(30);
        if (frontier === 'tick') f.measure();
        window.requestAnimationFrame = () => {
          throw cause;
        };
        assert.throws(
          () => (frontier === 'motion' ? f.owner.pointermove(pointer(1, 119)) : f.measure()),
          (e) => e === cause,
        );
      }
      assert.equal(f.owner.blocksInput(), false);
      if (frontier === 'tick') {
        assert.equal(f.captures.size, 1);
        drain(f);
      }
      assert.equal(f.captures.size, 0);
      assert.equal(f.frames.size, 0);
      assert.equal(f.removed, 1);
      while (f.tasks.length) f.task();
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      window.requestAnimationFrame = raf;
      f.destroy();
    }
  });

test('RAF failure retains ordered pointer-release cleanup cause while releasing all ownership', () => {
  const f = fixture(),
    primary = new Error('RAF scheduling failure'),
    secondary = new Error('capture cleanup failure'),
    window = f.view.dom.ownerDocument.defaultView!,
    raf = window.requestAnimationFrame,
    release = f.view.contentDOM.releasePointerCapture;
  try {
    f.setChipGeometry(0, 15, 119);
    f.owner.pointerdown(pointer(1, 119));
    drain(f);
    dragPointer(f.owner, pointer(1, 119));
    // Deliver actual cold admission before exercising the existing RAF fault boundary.
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 119, 16));
    f.frame(30);
    window.requestAnimationFrame = () => {
      throw primary;
    };
    f.view.contentDOM.releasePointerCapture = (id) => {
      release.call(f.view.contentDOM, id);
      throw secondary;
    };
    assert.throws(
      () => f.owner.pointermove(pointer(1, 119)),
      (cause) => {
        assert.equal(cause instanceof AggregateError, true);
        assert.deepEqual((cause as AggregateError).errors, [primary, secondary]);
        return true;
      },
    );
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.removed, 1);
    while (f.tasks.length) f.task();
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    window.requestAnimationFrame = raf;
    f.view.contentDOM.releasePointerCapture = release;
    f.destroy();
  }
});

for (const reentry of [false, true])
  for (const cleanupFails of [false, true])
    test(`entry failure preserves cause with cleanup failure ${cleanupFails} and newer entry ${reentry}`, () => {
      const f = fixture(),
        primary = new Error('entry finish failure'),
        secondary = new Error('entry cancel failure');
      let reservations = 0;
      const cancels = [0, 0];
      try {
        f.view.dispatch({
          effects: StateEffect.appendConfig.of(
            nativeInputSettlement.of({
              hasCompositionInput: () => false,
              cancelCompositionInput() {},
              hasHeldKey: () => false,
              reserveCommand: () => {
                const id = reservations++;
                return {
                  finish: () => {
                    if (id === 0) throw primary;
                  },
                  cancel: () => {
                    cancels[id] = (cancels[id] ?? 0) + 1;
                    if (id === 0) {
                      if (reentry) assert.equal(f.owner.keydown(key('Alt')), true);
                      if (cleanupFails) throw secondary;
                    }
                  },
                };
              },
            }),
          ),
        });
        const selection = f.state.selection;
        f.owner.keydown(key('Alt'));
        f.measure();
        assert.throws(
          () => f.task(),
          (cause) => {
            if (!cleanupFails) return cause === primary;
            assert.equal(cause instanceof AggregateError, true);
            assert.deepEqual((cause as AggregateError).errors, [primary, secondary]);
            return true;
          },
        );
        assert.equal(cancels[0], 1);
        assert.equal(readExactSource(f.state), 'a, b, c');
        assert.equal(f.state.selection, selection);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(f.state.field(reorderPreview).value, null);
        if (reentry) {
          assert.equal(f.owner.blocksInput(), true);
          assert.equal(reservations, 2);
          f.measure();
          f.task();
          assert.notEqual(f.state.field(reorderPreview).value, null);
          assert.equal(f.owner.blocksInput(), true);
          f.owner.keydown(key('Escape'));
          assert.equal(cancels[1], 1);
        }
        assert.equal(f.owner.blocksInput(), false);
        assert.equal(f.state.field(reorderPreview).value, null);
        assert.equal(cancels[0], 1);
        assert.equal(f.frames.size, 0);
        assert.equal(f.captures.size, 0);
        assert.equal(undoDepth(f.state), 0);
      } finally {
        f.destroy();
      }
    });

test('RAF failure cleanup preserves a genuinely admitted newer entry and its measurement epoch', () => {
  const f = fixture(),
    cause = new Error('RAF scheduling failure'),
    window = f.view.dom.ownerDocument.defaultView!,
    raf = window.requestAnimationFrame;
  let reservations = 0;
  const cancels = [0, 0];
  try {
    f.view.dispatch({
      effects: StateEffect.appendConfig.of(
        nativeInputSettlement.of({
          hasCompositionInput: () => false,
          cancelCompositionInput() {},
          hasHeldKey: () => false,
          reserveCommand: () => {
            const id = reservations++;
            return {
              finish() {},
              cancel() {
                cancels[id] = (cancels[id] ?? 0) + 1;
                if (id === 0) assert.equal(f.owner.keydown(key('Alt')), true);
              },
            };
          },
        }),
      ),
    });
    f.setChipGeometry(0, 15, 119);
    f.owner.pointerdown(pointer(1, 119));
    drain(f);
    dragPointer(f.owner, pointer(1, 119));
    // Deliver actual cold admission before exercising the existing RAF fault boundary.
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 119, 16));
    f.frame(30);
    window.requestAnimationFrame = () => {
      throw cause;
    };
    assert.throws(
      () => f.owner.pointermove(pointer(1, 119)),
      (e) => e === cause,
    );
    assert.equal(cancels[0], 1);
    assert.equal(f.owner.blocksInput(), true);
    assert.equal(f.frames.size, 0);
    assert.equal(f.captures.size, 0);
    assert.equal(f.removed, 1);
    while (f.tasks.length) f.task();
    assert.equal(f.jobs.length, 2);
    drain(f);
    assert.notEqual(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), true);
    f.owner.keydown(key('Escape'));
    assert.deepEqual(cancels, [1, 1]);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    window.requestAnimationFrame = raf;
    f.destroy();
  }
});

test("old RAF surviving failed cancellation cannot clear a newer pointer gesture's frame ownership", () => {
  const f = fixture(),
    cause = new Error('failed old RAF release'),
    window = f.view.dom.ownerDocument.defaultView!,
    cancel = window.cancelAnimationFrame;
  const selection = f.state.selection;
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 20, 16));
    assert.equal(f.frames.size, 2);
    const retiredCallbacks = [...f.frames];
    window.cancelAnimationFrame = () => {
      throw cause;
    };
    assert.throws(
      () => f.owner.close(),
      (e) =>
        e instanceof AggregateError &&
        e.errors.length === 2 &&
        e.errors[0] === cause &&
        e.errors[1] === cause,
    );
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.removed, 1);
    assert.equal(f.frames.size, 2);
    window.cancelAnimationFrame = cancel;
    f.measure();
    f.task();
    f.owner.pointerdown(pointer(2));
    drain(f);
    dragPointer(f.owner, pointer(2));
    f.measure();
    f.task();
    f.owner.pointermove(pointer(2, 20, 16));
    assert.equal(f.frames.size, 4);
    assert.equal(f.captures.size, 1);
    for (const [handle, callback] of retiredCallbacks) {
      f.frames.delete(handle);
      callback(30);
    }
    assert.equal(f.frames.size, 2);
    assert.equal(f.owner.blocksInput(), true);
    f.owner.close();
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    assert.equal(f.removed, 2);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    f.measure();
    f.task();
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    window.cancelAnimationFrame = cancel;
    f.destroy();
  }
});
