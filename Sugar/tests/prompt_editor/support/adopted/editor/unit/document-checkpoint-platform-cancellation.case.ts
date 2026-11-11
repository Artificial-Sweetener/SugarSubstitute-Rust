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
/** Genuine platform cancellation remains authoritative after observable final signal reads. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './document-checkpoint-lifetime-fixture.ts';

/** Preserve genuine cancellation state while controlling only the final observable accessor. */
function finalSignal(transition: () => void) {
  const controller = new AbortController();
  const signal = controller.signal;
  const read = Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted')!.get!;
  const remove = signal.removeEventListener.bind(signal);
  let armed = false;
  let reads = 0;
  Object.defineProperties(signal, {
    removeEventListener: {
      configurable: true,
      value(...args: Parameters<AbortSignal['removeEventListener']>) {
        remove(...args);
        armed = true;
      },
    },
    aborted: {
      configurable: true,
      get() {
        const prior: boolean = read.call(signal);
        if (armed && reads++ === 0) transition();
        return prior;
      },
    },
  });
  return {
    signal,
    controller,
    canonical: () => read.call(signal) as boolean,
    reads: () => reads,
    restore() {
      Reflect.deleteProperty(signal, 'aborted');
      Reflect.deleteProperty(signal, 'removeEventListener');
    },
  };
}

for (const transition of ['abort', 'dispose', 'supersede', 'edit'] as const) {
  test(`final genuine signal read preserves ${transition} authority`, async () => {
    const f = fixture();
    const observed = finalSignal(() => {
      observed.controller.abort();
      if (transition === 'dispose') f.dispose();
      else if (transition === 'supersede') f.owner.supersede();
      else if (transition === 'edit')
        f.transport.dispatch(f.transport.state.update({ changes: { from: 0, insert: 'x' } }));
    });
    try {
      const pending = f.owner.capture(observed.signal);
      f.flush();
      const result = await pending;
      if (result.status === 'captured') result.checkpoint.release();
      assert.deepEqual(result, {
        status: 'rejected',
        reason:
          transition === 'abort'
            ? 'cancelled'
            : transition === 'dispose'
              ? 'disposed'
              : 'superseded',
      });
      assert.equal(observed.canonical(), true);
      assert.equal(observed.reads(), 1);
      assert.equal(f.timers.size, 0);
      assert.equal(f.counts().cancelled, 1);
      assert.equal(f.counts().cleared, 1);
    } finally {
      observed.restore();
      f.dispose();
    }
  });
}

test('ordinary genuine signal still captures and releases exact native source', async () => {
  const f = fixture();
  const observed = finalSignal(() => {});
  try {
    const pending = f.owner.capture(observed.signal);
    f.flush();
    const result = await pending;
    assert.equal(result.status, 'captured');
    if (result.status === 'captured') {
      assert.equal(result.checkpoint.source, 'current');
      result.checkpoint.release();
      assert.equal(f.owner.inspect(result.checkpoint), 'released-checkpoint');
    }
    assert.equal(observed.canonical(), false);
    assert.equal(observed.reads(), 1);
    assert.equal(f.timers.size, 0);
  } finally {
    observed.restore();
    f.dispose();
  }
});

test('final signal accessor thrown undefined remains the exact unexpected cause', async () => {
  const f = fixture();
  const observed = finalSignal(() => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  });
  try {
    const pending = f.owner.capture(observed.signal);
    f.flush();
    let failed = false;
    await pending.catch((cause: unknown) => {
      failed = true;
      assert.equal(cause, undefined);
    });
    assert.equal(failed, true);
    assert.equal(f.timers.size, 0);
    assert.equal(f.counts().cancelled, 1);
    assert.equal(f.counts().cleared, 1);
  } finally {
    observed.restore();
    f.dispose();
  }
});

test('genuine cancellation never hides ordered independent cleanup failures', async () => {
  const f = fixture();
  const second = new Error('controlled cleanup');
  f.cleanup.set('input', () => {
    f.abort();
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  });
  f.cleanup.set('timer', () => {
    throw second;
  });
  try {
    const pending = f.owner.capture(f.signal);
    f.flush();
    await assert.rejects(pending, (cause: unknown) => {
      assert.ok(cause instanceof AggregateError);
      assert.deepEqual(cause.errors, [undefined, second]);
      return true;
    });
    assert.equal(f.listeners.size, 0);
    assert.equal(f.counts().removed, 1);
    assert.equal(f.counts().cleared, 1);
  } finally {
    f.timers.clear();
    f.dispose();
  }
});
