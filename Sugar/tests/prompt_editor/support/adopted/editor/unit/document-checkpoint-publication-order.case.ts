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
/** Final capture observes external accessors before current native readiness and publication authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './document-checkpoint-lifetime-fixture.ts';
import { checkpointFixture } from '../transaction/document-checkpoint-fixture.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';

/** Arm one genuine caller accessor only after the capture's abort listener has been released. */
function signalBoundary(callback: () => void) {
  const controller = new AbortController();
  const signal = controller.signal;
  const read = Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted')!.get!;
  const remove = signal.removeEventListener.bind(signal);
  let armed = false;
  let observed = false;
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
        if (armed && !observed) {
          observed = true;
          callback();
        }
        return prior;
      },
    },
  });
  return {
    controller,
    signal,
    observed: () => observed,
    restore() {
      Reflect.deleteProperty(signal, 'aborted');
      Reflect.deleteProperty(signal, 'removeEventListener');
    },
  };
}

test('final signal accessor beginning native composition rejects publication', async () => {
  const f = fixture();
  const boundary = signalBoundary(() => {
    f.transport.compositionStarted = true;
  });
  try {
    const pending = f.owner.capture(boundary.signal);
    f.flush();
    const result = await pending;
    if (result.status === 'captured') result.checkpoint.release();
    assert.deepEqual(result, { status: 'rejected', reason: 'composing' });
    assert.equal(boundary.observed(), true);
    assert.equal(f.transport.compositionStarted, true);
    assert.equal(f.timers.size, 0);
  } finally {
    boundary.restore();
    f.dispose();
  }
});

for (const transition of ['abort', 'source', 'dispose', 'supersede', 'undefined'] as const) {
  test(`final native composition callback preserves ${transition} authority`, async () => {
    const f = fixture();
    let callbacks = 0;
    const boundary = signalBoundary(() => {
      f.hooks.set('composition', () => {
        f.hooks.delete('composition');
        callbacks++;
        if (transition === 'abort') boundary.controller.abort();
        else if (transition === 'source')
          f.transport.dispatch(f.transport.state.update({ changes: { from: 0, insert: 'x' } }));
        else if (transition === 'dispose') f.dispose();
        else if (transition === 'supersede') f.owner.supersede();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        else throw undefined;
      });
    });
    try {
      const pending = f.owner.capture(boundary.signal);
      f.flush();
      if (transition === 'undefined') {
        let failed = false;
        await pending.catch((cause: unknown) => {
          failed = true;
          assert.equal(cause, undefined);
        });
        assert.equal(failed, true);
      } else {
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
      }
      assert.equal(callbacks, 1);
      assert.equal(boundary.observed(), true);
      assert.equal(f.timers.size, 0);
      assert.equal(f.counts().cancelled, 1);
      assert.equal(f.counts().cleared, 1);
    } finally {
      boundary.restore();
      f.dispose();
    }
  });
}

test('final native composition callback changing genuine context rejects publication', async () => {
  const f = checkpointFixture();
  let armed = false;
  let callbacks = 0;
  const boundary = signalBoundary(() => {
    armed = true;
  });
  Object.assign(f.transport, {
    plugin: () => ({
      hasCompositionInput: () => {
        if (armed) {
          armed = false;
          callbacks++;
          f.transport.dispatch({ effects: invalidateCommandContext.of(null) });
        }
        return false;
      },
      reserveCommand: () => ({ finish() {}, cancel() {} }),
    }),
  });
  try {
    const pending = f.updates.captureDocument(boundary.signal);
    f.flush();
    const result = await pending;
    if (result.status === 'captured') result.checkpoint.release();
    assert.deepEqual(result, { status: 'rejected', reason: 'superseded' });
    assert.equal(callbacks, 1);
    assert.equal(boundary.observed(), true);
  } finally {
    boundary.restore();
    f.dispose();
  }
});
