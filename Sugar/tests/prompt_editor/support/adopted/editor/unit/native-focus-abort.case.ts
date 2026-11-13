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
/** Canonical request cancellation owns finite abort listeners without retiring another caller's readiness. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { NativeFocus } from '../../../../../../frontend/sugarprompt/editor/src/native-focus.ts';

/** Instrument real abort delivery while preserving the actual native EventTarget methods. */
function trackedSignal() {
  const controller = new AbortController(),
    listeners = new Set<EventListenerOrEventListenerObject>();
  const signal = controller.signal,
    add = signal.addEventListener,
    remove = signal.removeEventListener;
  signal.addEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ) => {
    if (type === 'abort' && listener) listeners.add(listener);
    add.call(signal, type, listener, options);
  };
  signal.removeEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ) => {
    if (type === 'abort' && listener) listeners.delete(listener);
    remove.call(signal, type, listener, options);
  };
  return { controller, signal, listeners };
}

/** Public view and clock boundaries supply controlled measurements only; the real owner decides all readiness and cleanup. */
function fixture() {
  const timers = new Map<number, () => void>(),
    measures: Array<() => void> = [];
  let id = 0,
    focused = false,
    focuses = 0,
    reads = 0;
  const hooks: { dispatch?: () => void; clear?: () => void } = {};
  const transport = {
    state: EditorState.create({ doc: 'alpha' }),
    root: { activeElement: null },
    dom: { contains: () => false },
    get hasFocus() {
      return focused;
    },
    scrollDOM: {
      get clientHeight() {
        reads++;
        return 50;
      },
    },
    dispatch() {
      hooks.dispatch?.();
    },
    focus() {
      focused = true;
      focuses++;
    },
    requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
      measures.push(() => request.write?.(request.read(view), view));
    },
  };
  const view = transport as unknown as EditorView;
  const owner = new NativeFocus(view, {
    set(callback) {
      timers.set(++id, callback);
      return id;
    },
    clear(handle) {
      timers.delete(handle as number);
      hooks.clear?.();
    },
  });
  return {
    owner,
    timers,
    hooks,
    counts: () => ({ focuses, reads, queued: measures.length }),
    deadline() {
      for (const callback of [...timers.values()]) callback();
    },
    /** Deliver only the owner's bounded read/write callbacks and their publication microtasks. */
    async flush() {
      let passes = 0;
      while (measures.length) {
        assert.ok(++passes <= 16);
        measures.shift()!();
        await Promise.resolve();
      }
    },
  };
}

test('matching cancellation identity coalesces the actual promise and success removes its listener', async () => {
  const native = fixture(),
    cancellation = trackedSignal();
  const first = native.owner.focus('preserve-viewport', cancellation.signal);
  assert.equal(native.owner.focus('preserve-viewport', cancellation.signal), first);
  assert.equal(cancellation.listeners.size, 1);
  await native.flush();
  assert.equal(await first, true);
  assert.equal(cancellation.listeners.size, 0);
  assert.equal(native.timers.size, 0);
  cancellation.controller.abort();
  assert.equal(native.counts().focuses, 1);
  native.owner.dispose();
});

test('another signal supersedes rather than coalesces; the old abort cannot retire its newer request', async () => {
  const native = fixture(),
    older = trackedSignal(),
    newer = trackedSignal();
  const first = native.owner.focus('preserve-viewport', older.signal);
  const second = native.owner.focus('preserve-viewport', newer.signal);
  assert.notEqual(first, second);
  assert.equal(await first, false);
  assert.equal(older.listeners.size, 0);
  older.controller.abort();
  assert.equal(newer.listeners.size, 1);
  await native.flush();
  assert.equal(await second, true);
  assert.equal(newer.listeners.size, 0);
  native.owner.dispose();
});

for (const terminal of ['abort', 'deadline', 'retire', 'dispose'] as const)
  test(`${terminal} settles cancellation and makes late measurements inert`, async () => {
    const native = fixture(),
      cancellation = trackedSignal();
    const pending = native.owner.focus('preserve-viewport', cancellation.signal);
    if (terminal === 'abort') cancellation.controller.abort();
    else if (terminal === 'deadline') native.deadline();
    else native.owner[terminal]();
    assert.equal(await pending, false);
    assert.equal(cancellation.listeners.size, 0);
    assert.equal(native.timers.size, 0);
    await native.flush();
    assert.equal(native.counts().reads, 0);
    native.owner.dispose();
  });

test('pre-aborted cancellation owns no focus, listener, timer or measurement', async () => {
  const native = fixture(),
    cancellation = trackedSignal();
  cancellation.controller.abort();
  assert.equal(await native.owner.focus('preserve-viewport', cancellation.signal), false);
  assert.equal(cancellation.listeners.size, 0);
  assert.equal(native.timers.size, 0);
  assert.deepEqual(native.counts(), { focuses: 0, reads: 0, queued: 0 });
  native.owner.dispose();
});

test('synchronous dispatch reentry supersedes only the older request and its signal', async () => {
  const native = fixture(),
    older = trackedSignal(),
    newer = trackedSignal();
  let continued: Promise<boolean> | undefined;
  native.hooks.dispatch = () => {
    delete native.hooks.dispatch;
    continued = native.owner.focus('preserve-viewport', newer.signal);
  };
  const first = native.owner.focus('reveal-selection', older.signal);
  assert.equal(await first, false);
  assert.equal(older.listeners.size, 0);
  older.controller.abort();
  await native.flush();
  assert.equal(await continued, true);
  assert.equal(newer.listeners.size, 0);
  assert.equal(native.counts().focuses, 1);
  native.owner.dispose();
});

test('a thrown undefined cause survives cleanup failure after the exact abort listener and timer release', () => {
  const native = fixture(),
    cancellation = trackedSignal(),
    cleanup = {};
  native.hooks.dispatch = () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  };
  native.hooks.clear = () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw cleanup;
  };
  let thrown: unknown;
  try {
    native.owner.focus('reveal-selection', cancellation.signal);
  } catch (cause) {
    thrown = cause;
  }
  assert.ok(thrown instanceof AggregateError);
  assert.equal(thrown.errors[0], undefined);
  assert.equal(thrown.errors[1], cleanup);
  assert.equal(cancellation.listeners.size, 0);
  assert.equal(native.timers.size, 0);
  delete native.hooks.clear;
  native.owner.dispose();
});

test('listener registration failure preserves thrown undefined and releases the actual timer and installed listener', () => {
  const native = fixture(),
    cancellation = trackedSignal();
  const add = cancellation.signal.addEventListener;
  cancellation.signal.addEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ) => {
    add.call(cancellation.signal, type, listener, options);
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  };
  let rejected = false,
    caught: unknown;
  try {
    native.owner.focus('preserve-viewport', cancellation.signal);
  } catch (cause) {
    rejected = true;
    caught = cause;
  }
  assert.equal(rejected, true);
  assert.equal(caught, undefined);
  assert.equal(cancellation.listeners.size, 0);
  assert.equal(native.timers.size, 0);
  assert.equal(native.counts().focuses, 0);
  native.owner.dispose();
});

test('listener and timer cleanup failures remain ordered while the request settles once', async () => {
  const native = fixture(),
    cancellation = trackedSignal(),
    timerFailure = {};
  const remove = cancellation.signal.removeEventListener;
  cancellation.signal.removeEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ) => {
    remove.call(cancellation.signal, type, listener, options);
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  };
  const pending = native.owner.focus('preserve-viewport', cancellation.signal);
  native.hooks.clear = () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw timerFailure;
  };
  let caught: unknown;
  try {
    native.owner.retire();
  } catch (cause) {
    caught = cause;
  }
  assert.ok(caught instanceof AggregateError);
  assert.equal(caught.errors[0], undefined);
  assert.equal(caught.errors[1], timerFailure);
  assert.equal(await pending, false);
  assert.equal(cancellation.listeners.size, 0);
  assert.equal(native.timers.size, 0);
  delete native.hooks.clear;
  cancellation.controller.abort();
  await native.flush();
  assert.equal(native.counts().reads, 0);
  native.owner.dispose();
});
