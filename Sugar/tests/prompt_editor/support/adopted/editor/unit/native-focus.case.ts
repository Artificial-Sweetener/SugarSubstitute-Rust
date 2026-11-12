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
/** Controlled native measurement/timer transport proves finite owner work without browser geometry claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import {
  NativeFocus,
  type FocusClock,
} from '../../../../../../frontend/sugarprompt/editor/src/native-focus.ts';

/** Supported native measurement request shape, derived from the public method rather than a private type. */
type NativeMeasure = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** Drive the actual focus owner through its supported dispatch/measure seam. */
function fixture() {
  const timers = new Map<number, () => void>();
  let id = 0,
    focused = false,
    height = 50,
    reads = 0,
    fail = false,
    scheduled = 0,
    dispatches = 0;
  const measurements: NativeMeasure[] = [];
  const clock: FocusClock = {
    set(callback, milliseconds) {
      assert.equal(milliseconds, 1000);
      timers.set(++id, callback);
      return id;
    },
    clear(handle) {
      timers.delete(handle as number);
    },
  };
  const transport = {
    state: EditorState.create({ doc: 'current' }),
    root: { activeElement: null },
    dom: { contains: () => false },
    get hasFocus() {
      return focused;
    },
    scrollDOM: {
      get clientHeight() {
        reads++;
        return height;
      },
    },
    dispatch() {
      dispatches++;
      if (fail) throw new Error('synthetic-dispatch-failure');
    },
    focus() {
      focused = true;
    },
    requestMeasure(request: NativeMeasure) {
      scheduled++;
      measurements.push(request);
    },
  };
  // No geometry/DOM implementation is substituted: the seam supplies literal measurements only.
  const owner = new NativeFocus(transport as unknown as EditorView, clock);
  return {
    owner,
    timers,
    transport,
    failure(value: boolean) {
      fail = value;
    },
    departure() {
      focused = false;
    },
    counts: () => ({ reads, scheduled, timers: timers.size }),
    dispatches: () => dispatches,
    /** One literal native measurement completes before queued host work is observed. */
    async pass(nextHeight: number) {
      height = nextHeight;
      const request = measurements.shift();
      assert.ok(request, 'native measurement unavailable');
      const value = request.read(transport as unknown as EditorView);
      request.write?.(value, transport as unknown as EditorView);
      await Promise.resolve();
    },
    /** The controlled deadline performs no geometry read and cannot grab focus. */
    deadline() {
      for (const callback of [...timers.values()]) callback();
    },
  };
}

test('focused calls share actual pending promise; settled viewport uses no more than eight passes and releases deadline', async () => {
  const native = fixture();
  const first = native.owner.focus();
  assert.strictEqual(native.owner.focus(), first);
  await native.pass(50);
  await native.pass(50);
  await native.pass(50);
  assert.equal(await first, true);
  assert.deepEqual(native.counts(), { reads: 3, scheduled: 3, timers: 0 });
});

test('alternating geometry exhausts exactly eight passes and releases owned timer', async () => {
  const native = fixture(),
    pending = native.owner.focus();
  for (let pass = 0; pass < 8; pass++) await native.pass(pass % 2 ? 50 : 60);
  assert.equal(await pending, false);
  assert.deepEqual(native.counts(), { reads: 8, scheduled: 8, timers: 0 });
});

test('viewport-preserving restoration focuses with finite readiness and never dispatches a caret reveal', async () => {
  const native = fixture();
  const pending = native.owner.focus('preserve-viewport');
  assert.equal(native.dispatches(), 0);
  await native.pass(50);
  await native.pass(50);
  assert.equal(await pending, true);
  assert.equal(native.dispatches(), 0);
  assert.deepEqual(native.counts(), { reads: 2, scheduled: 2, timers: 0 });
});

test('deadline, retirement and disposal settle false; late measurements read no geometry', async () => {
  for (const end of ['deadline', 'retire', 'dispose'] as const) {
    const native = fixture(),
      pending = native.owner.focus();
    if (end === 'deadline') native.deadline();
    else native.owner[end]();
    assert.equal(await pending, false);
    await native.pass(50);
    assert.deepEqual(native.counts(), { reads: 0, scheduled: 1, timers: 0 });
  }
});

test('focus departure retires prior request; superseding reply cannot complete newer readiness', async () => {
  const native = fixture(),
    first = native.owner.focus();
  native.departure();
  const second = native.owner.focus();
  assert.equal(await first, false);
  await native.pass(50);
  await native.pass(50);
  await native.pass(50);
  await native.pass(50);
  assert.equal(await second, true);
  assert.equal(native.counts().timers, 0);
});

test('unexpected dispatch failure remains observable and releases work before a subsequent valid request', async () => {
  const native = fixture();
  native.failure(true);
  assert.throws(() => native.owner.focus(), /synthetic-dispatch-failure/u);
  assert.equal(native.counts().timers, 0);
  native.failure(false);
  const pending = native.owner.focus();
  await native.pass(50);
  await native.pass(50);
  await native.pass(50);
  assert.equal(await pending, true);
});
