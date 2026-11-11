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
/** Independent numeric frames exercise the actual finite clock without DOM, source or native geometry substitutes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ReorderMotionClock,
  type ReorderMotionFrame,
  type ReorderMotionTarget,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-clock.ts';
/** Controlled native timing retains genuine scheduled callbacks and exact thrown causes. */
function fixture() {
  let now = 0,
    next = 0,
    current = true,
    settled = 0;
  const callbacks = new Map<number, () => void>(),
    frames: ReorderMotionFrame[] = [];
  const clock = new ReorderMotionClock({
    now: () => now,
    request: (callback) => {
      callbacks.set(++next, callback);
      return next;
    },
    cancel: (handle) => {
      callbacks.delete(handle);
    },
    current: () => current,
    paint: (frame) => {
      frames.push(frame);
    },
    settled: () => {
      ++settled;
    },
    failed: (cause) => {
      throw cause;
    },
  });
  return {
    clock,
    callbacks,
    frames,
    get settled() {
      return settled;
    },
    stale() {
      current = false;
    },
    step(time: number) {
      now = time;
      const pending = [...callbacks.values()];
      callbacks.clear();
      for (const callback of pending) callback();
    },
  };
}
const targets: readonly ReorderMotionTarget[] = Object.freeze([
  {
    id: 0,
    kind: 'peer',
    from: { left: 10, top: 20 },
    to: { left: 110, top: 60 },
  },
  {
    id: 1,
    kind: 'held',
    from: { left: 30, top: 80 },
    to: { left: 190, top: 0 },
  },
]);
test('180ms peer OutQuad and keyboard held OutCubic share one literal coherent frame', () => {
  const f = fixture();
  f.clock.replace(targets, false);
  assert.deepEqual(f.frames[0]?.positions, [
    { id: 0, left: 10, top: 20 },
    { id: 1, left: 30, top: 80 },
  ]);
  assert.equal(f.callbacks.size, 1);
  f.step(90);
  assert.deepEqual(f.frames[1], {
    progress: 0.5,
    positions: [
      { id: 0, left: 85, top: 50 },
      { id: 1, left: 170, top: 10 },
    ],
  });
  f.step(180);
  assert.deepEqual(f.frames[2]?.positions, [
    { id: 0, left: 110, top: 60 },
    { id: 1, left: 190, top: 0 },
  ]);
  assert.equal(f.settled, 1);
  assert.equal(f.callbacks.size, 0);
  assert.ok(Object.isFrozen(f.frames[1]));
  assert.ok(Object.isFrozen(f.frames[1]?.positions[0]));
});
test('retarget starts at last published poses and never clears a coherent display frame', () => {
  const f = fixture();
  f.clock.replace(targets, false);
  f.step(90);
  const published = f.clock.positions();
  f.clock.replace(
    [
      {
        id: 0,
        kind: 'peer',
        from: { left: -900, top: -900 },
        to: { left: 5, top: 10 },
      },
    ],
    false,
  );
  assert.deepEqual(f.frames.at(-1)?.positions, [{ id: 0, left: 85, top: 50 }]);
  assert.equal(published[0]?.left, 85);
  assert.equal(f.callbacks.size, 1);
  f.step(180);
  assert.deepEqual(f.frames.at(-1)?.positions, [{ id: 0, left: 25, top: 20 }]);
  f.step(270);
  assert.deepEqual(f.frames.at(-1)?.positions, [{ id: 0, left: 5, top: 10 }]);
});
test('reduced motion and stale/destroyed owners never leave live callbacks', () => {
  const reduced = fixture();
  reduced.clock.replace(targets, true);
  assert.equal(reduced.frames.length, 1);
  assert.equal(reduced.frames[0]?.progress, 1);
  assert.equal(reduced.settled, 1);
  assert.equal(reduced.callbacks.size, 0);
  const stale = fixture();
  stale.clock.replace(targets, false);
  const callback = [...stale.callbacks.values()][0]!;
  stale.stale();
  stale.step(90);
  assert.equal(stale.frames.length, 1);
  assert.equal(stale.callbacks.size, 0);
  stale.clock.destroy();
  callback();
  stale.clock.replace(targets, false);
  assert.equal(stale.frames.length, 1);
});
test('invalid geometry preserves the live prior clock and exact owner failure causes remain observable', () => {
  const f = fixture();
  f.clock.replace(targets, false);
  assert.throws(
    () =>
      f.clock.replace(
        [
          {
            id: 0,
            kind: 'peer',
            from: { left: NaN, top: 0 },
            to: { left: 1, top: 1 },
          },
        ],
        false,
      ),
    /reorder-motion-geometry/,
  );
  assert.equal(f.callbacks.size, 1);
  const cause = new Error('paint-boundary'),
    cleanup = new Error('retirement-boundary');
  let callback: (() => void) | null = null,
    now = 0;
  const clock = new ReorderMotionClock({
    now: () => now,
    request: (action) => {
      callback = action;
      return 1;
    },
    cancel: () => {
      throw cleanup;
    },
    current: () => true,
    paint: () => {
      if (now) throw cause;
    },
    settled() {},
    failed: (error) => {
      throw error;
    },
  });
  clock.replace(targets, false);
  now = 90;
  assert.throws(
    () => (callback as unknown as () => void)(),
    (error) => error === cause,
  );
  // The running callback's handle was consumed before paint; no unowned cancellation is attempted.
  assert.equal(callback !== null, true);
});
test('reentrant paint replacement owns the new callback and old completion cannot settle it', () => {
  let replaced = false,
    settled = 0,
    next = 0;
  const pending = new Map<number, () => void>(),
    frames: ReorderMotionFrame[] = [];
  const clock = new ReorderMotionClock({
    now: () => 0,
    request: (callback) => {
      pending.set(++next, callback);
      return next;
    },
    cancel: (handle) => {
      pending.delete(handle);
    },
    current: () => true,
    paint(frame) {
      frames.push(frame);
      if (!replaced) {
        replaced = true;
        clock.replace([targets[1]!], false);
      }
    },
    settled() {
      ++settled;
    },
    failed: (cause) => {
      throw cause;
    },
  });
  clock.replace(targets, false);
  assert.equal(frames.length, 2);
  assert.equal(pending.size, 1);
  assert.equal(settled, 0);
  clock.destroy();
  assert.equal(pending.size, 0);
});
