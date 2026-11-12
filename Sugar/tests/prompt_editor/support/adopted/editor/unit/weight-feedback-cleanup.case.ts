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
/** Actual passive preview retirement attempts each owned release and preserves transport causes. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { previewFixture } from './weight-feedback-fixture.ts';

for (const mask of [1, 2, 3, 4, 8, 15])
  for (const undefinedCause of [false, true])
    test(`terminal preview cleanup attempts every release and keeps retained callbacks inert, mask=${mask}, undefined=${undefinedCause}`, async () => {
      const f = previewFixture(),
        clock = f.view.dom.ownerDocument.defaultView!,
        clear = clock.clearTimeout,
        removeListener = clock.removeEventListener,
        removePortal = f.root.remove,
        attempts: string[] = [],
        causes: unknown[] = [
          undefinedCause ? undefined : new Error('controlled-clear'),
          new Error('controlled-hide'),
          new Error('controlled-listener'),
          new Error('controlled-portal'),
        ];
      let hidden = f.root.hidden;
      try {
        f.owner.show(f.session(), { x: 100, y: 90 });
        const pending = f.jobs[0]!;
        f.measure();
        f.measure();
        const [timerId, timer] = [...f.timers][0]!,
          blur = f.listeners.get('blur')!;
        assert.equal(timer.delay, 240);
        clock.clearTimeout = (id): void => {
          attempts.push('timer');
          if (mask & 1) throw causes[0];
          clear.call(clock, id);
        };
        Object.defineProperty(f.root, 'hidden', {
          configurable: true,
          get: () => hidden,
          set(value: boolean) {
            attempts.push('hide');
            if (mask & 2) throw causes[1];
            hidden = value;
          },
        });
        clock.removeEventListener = (): void => {
          attempts.push('listener');
          if (mask & 4) throw causes[2];
          removeListener.call(clock, 'blur', blur);
        };
        f.root.remove = (): void => {
          attempts.push('portal');
          if (mask & 8) throw causes[3];
          removePortal();
        };
        let caught = false,
          failure: unknown;
        try {
          f.owner.destroy();
        } catch (cause) {
          caught = true;
          failure = cause;
        }
        assert.equal(caught, true);
        const expected = causes.filter((_cause, index) => mask & (1 << index));
        if (expected.length === 1) assert.equal(failure, expected[0]);
        else {
          assert.ok(failure instanceof AggregateError);
          assert.deepEqual(failure.errors, expected);
        }
        assert.deepEqual(attempts, ['timer', 'hide', 'listener', 'portal']);
        assert.equal(f.timers.size, mask & 1 ? 1 : 0);
        assert.equal(f.listeners.size, mask & 4 ? 1 : 0);
        assert.equal(f.removed, mask & 8 ? 0 : 1);
        f.rejectObservations();
        await Promise.resolve();
        assert.equal(pending.read(f.view), null);
        pending.write?.(null, f.view);
        timer.callback();
        assert.equal(typeof blur, 'function');
        (blur as EventListener)(new Event('blur'));
        f.owner.clear();
        f.owner.destroy();
        assert.equal(f.jobs.length, 0);
        assert.deepEqual(attempts, ['timer', 'hide', 'listener', 'portal']);
        // The test transport owns disposal of failed external removals, not a runtime retry.
        f.timers.delete(timerId);
      } finally {
        clock.clearTimeout = clear;
        clock.removeEventListener = removeListener;
        f.root.remove = removePortal;
        Object.defineProperty(f.root, 'hidden', {
          configurable: true,
          writable: true,
          value: hidden,
        });
        for (const id of f.timers.keys()) clear.call(clock, id);
        removeListener.call(clock, 'blur', f.listeners.get('blur')!);
        if (!f.removed) removePortal();
        f.destroy();
      }
      assert.equal(f.timers.size + f.listeners.size, 0);
      assert.equal(f.removed, 1);
    });

for (const undefinedCause of [false, true])
  test(`measurement failure preserves the primary value before cleanup cause, undefined=${undefinedCause}`, () => {
    const f = previewFixture(),
      primary: unknown = undefinedCause ? undefined : new Error('controlled-measure'),
      cleanup = new Error('controlled-hide');
    let hidden = f.root.hidden;
    try {
      f.owner.show(f.session(), { x: 100, y: 90 });
      const pending = f.jobs.shift()!;
      globalThis.getComputedStyle = () => {
        throw primary;
      };
      Object.defineProperty(f.root, 'hidden', {
        configurable: true,
        get: () => hidden,
        set() {
          throw cleanup;
        },
      });
      assert.throws(
        () => pending.read(f.view),
        (cause) => {
          assert.ok(cause instanceof AggregateError);
          assert.deepEqual(cause.errors, [primary, cleanup]);
          return true;
        },
      );
      f.rejectObservations();
      assert.equal(pending.read(f.view), null);
      pending.write?.(null, f.view);
      assert.equal(f.jobs.length + f.timers.size, 0);
    } finally {
      hidden = true;
      Object.defineProperty(f.root, 'hidden', {
        configurable: true,
        writable: true,
        value: hidden,
      });
      f.destroy();
    }
  });
