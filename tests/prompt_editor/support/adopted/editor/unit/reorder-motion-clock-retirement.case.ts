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
/** Clock retirement preserves the current issued callback across synchronous native delegate reentry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ReorderMotionClock,
  type ReorderMotionFrame,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-clock.ts';

const first = [
    {
      id: 1,
      kind: 'peer' as const,
      from: { left: 10, top: 20 },
      to: { left: 110, top: 60 },
    },
  ],
  second = [
    {
      id: 1,
      kind: 'peer' as const,
      from: { left: -900, top: -900 },
      to: { left: 5, top: 10 },
    },
  ];
/** Actual clock callbacks and prepared endpoint identities are observed without simulating timing/easing rules. */
function fixture() {
  let now = 0,
    next = 0;
  const callbacks = new Map<number, () => void>(),
    frames: ReorderMotionFrame[] = [];
  const ports = {
    now: () => now,
    current: () => true,
    request: (callback: () => void) => {
      callbacks.set(++next, callback);
      return next;
    },
    cancel: (handle: number) => {
      callbacks.delete(handle);
    },
    paint: (frame: ReorderMotionFrame) => {
      frames.push(frame);
    },
    settled: () => {},
    failed: (cause: unknown): never => {
      throw cause;
    },
  };
  const clock = new ReorderMotionClock(ports);
  return {
    clock,
    ports,
    callbacks,
    frames,
    time: (value: number) => {
      now = value;
    },
    step: (value: number) => {
      now = value;
      const pending = [...callbacks.values()];
      callbacks.clear();
      for (const callback of pending) callback();
    },
  };
}

for (const frontier of ['cancel', 'now'] as const)
  for (const action of ['replace', 'destroy'] as const)
    test(`replacement ${frontier} ${action} cannot be overwritten by the older issue`, () => {
      const f = fixture();
      f.clock.replace(first, false);
      let entered = false;
      const cancel = f.ports.cancel,
        now = f.ports.now;
      const reenter = () => {
        if (entered) return;
        entered = true;
        if (action === 'replace') f.clock.replace(second, false);
        else f.clock.destroy();
      };
      if (frontier === 'cancel')
        f.ports.cancel = (handle) => {
          cancel(handle);
          reenter();
        };
      else
        f.ports.now = () => {
          if (entered) return now();
          reenter();
          return NaN;
        };
      if (frontier === 'now') f.time(90);
      f.clock.replace(first, false);
      assert.equal(entered, true);
      assert.equal(f.frames.length, action === 'replace' ? 2 : 1);
      assert.equal(f.callbacks.size, action === 'replace' ? 1 : 0);
      if (action === 'replace') {
        assert.deepEqual(f.clock.positions(), [{ id: 1, left: 10, top: 20 }]);
        f.step(270);
        assert.deepEqual(f.clock.positions(), [{ id: 1, left: 5, top: 10 }]);
      } else assert.deepEqual(f.clock.positions(), []);
      f.clock.destroy();
    });

for (const frontier of ['cancel', 'now'] as const)
  for (const cause of [null, undefined, new Error('replacement-native-raw')])
    test(`replacement ${frontier} preserves raw ${String(cause)} after acquiring a successor`, () => {
      const f = fixture(),
        cancel = f.ports.cancel,
        now = f.ports.now;
      f.clock.replace(first, false);
      let entered = false;
      const reenter = () => {
        entered = true;
        f.clock.replace(second, false);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      };
      if (frontier === 'cancel')
        f.ports.cancel = (handle) => {
          cancel(handle);
          if (!entered) reenter();
        };
      else
        f.ports.now = () => {
          if (!entered) reenter();
          return now();
        };
      if (frontier === 'now') f.time(90);
      assert.throws(
        () => f.clock.replace(first, false),
        (observed) => observed === cause,
      );
      assert.equal(f.callbacks.size, 1);
      assert.deepEqual(f.clock.positions(), [{ id: 1, left: 10, top: 20 }]);
      f.step(270);
      assert.deepEqual(f.clock.positions(), [{ id: 1, left: 5, top: 10 }]);
      f.clock.destroy();
    });

for (const result of [true, false])
  test(`advancing current replacement returning ${result} cannot publish or cancel the successor`, () => {
    const f = fixture();
    f.clock.replace(first, false);
    let entered = false;
    f.ports.current = () => {
      if (entered) return true;
      entered = true;
      f.clock.replace(second, false);
      return result;
    };
    f.step(90);
    assert.equal(f.frames.length, 2);
    assert.deepEqual(f.frames.at(-1)?.positions, [{ id: 1, left: 10, top: 20 }]);
    assert.equal(f.callbacks.size, 1);
    assert.equal(f.clock.positions(), f.frames.at(-1)?.positions);
    f.step(270);
    assert.deepEqual(f.frames.at(-1)?.positions, [{ id: 1, left: 5, top: 10 }]);
    f.clock.destroy();
  });

for (const action of ['replace', 'destroy'] as const)
  test(`advancing now ${action} rejects its stale invalid time before publication`, () => {
    const f = fixture();
    f.clock.replace(first, false);
    let entered = false;
    const now = f.ports.now;
    f.ports.now = () => {
      if (entered) return now();
      entered = true;
      if (action === 'replace') f.clock.replace(second, false);
      else f.clock.destroy();
      return NaN;
    };
    f.step(90);
    assert.equal(f.frames.length, action === 'replace' ? 2 : 1);
    assert.equal(f.callbacks.size, action === 'replace' ? 1 : 0);
    assert.deepEqual(
      f.clock.positions(),
      action === 'replace' ? [{ id: 1, left: 10, top: 20 }] : [],
    );
    f.clock.destroy();
  });

for (const action of ['replace', 'destroy'] as const)
  test(`late RAF handle after advancing ${action} is canceled without transferring ownership`, () => {
    const f = fixture(),
      request = f.ports.request,
      cancel = f.ports.cancel,
      canceled: number[] = [];
    f.clock.replace(first, false);
    let entered = false,
      late = 0;
    f.ports.cancel = (handle) => {
      canceled.push(handle);
      cancel(handle);
    };
    f.ports.request = (callback) => {
      const handle = request(callback);
      if (!entered) {
        entered = true;
        late = handle;
        if (action === 'replace') f.clock.replace(second, false);
        else f.clock.destroy();
      }
      return handle;
    };
    f.step(90);
    assert.deepEqual(canceled, [late]);
    assert.equal(f.callbacks.has(late), false);
    assert.equal(f.callbacks.size, action === 'replace' ? 1 : 0);
    if (action === 'replace') {
      assert.deepEqual(f.clock.positions(), [{ id: 1, left: 85, top: 50 }]);
      f.step(270);
      assert.deepEqual(f.clock.positions(), [{ id: 1, left: 5, top: 10 }]);
    } else assert.deepEqual(f.clock.positions(), []);
    f.clock.destroy();
  });

for (const cause of [null, undefined, new Error('late-RAF-cancel-raw')])
  test(`late RAF cancellation preserves raw ${String(cause)} and the successor callback`, () => {
    const f = fixture(),
      request = f.ports.request,
      cancel = f.ports.cancel;
    f.clock.replace(first, false);
    let entered = false,
      late = 0;
    f.ports.request = (callback) => {
      const handle = request(callback);
      if (!entered) {
        entered = true;
        late = handle;
        f.clock.replace(second, false);
      }
      return handle;
    };
    f.ports.cancel = (handle) => {
      cancel(handle);
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      if (handle === late) throw cause;
    };
    assert.throws(
      () => f.step(90),
      (observed) => observed === cause,
    );
    assert.equal(f.callbacks.has(late), false);
    assert.equal(f.callbacks.size, 1);
    assert.deepEqual(f.clock.positions(), [{ id: 1, left: 85, top: 50 }]);
    f.step(270);
    assert.deepEqual(f.clock.positions(), [{ id: 1, left: 5, top: 10 }]);
    f.clock.destroy();
  });
