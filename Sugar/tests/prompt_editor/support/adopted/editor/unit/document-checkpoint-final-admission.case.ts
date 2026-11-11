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
/** Final native checkpoint admission observes cleanup cancellation and real composition readiness. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { fixture } from './document-checkpoint-lifetime-fixture.ts';

for (const transition of ['abort', 'composition'] as const) {
  for (const release of ['input', 'timer', 'listener'] as const) {
    test(`final capture rejects ${transition} during ${release} cleanup`, async () => {
      const f = fixture();
      let callbacks = 0;
      try {
        f.cleanup.set(release, () => {
          callbacks++;
          if (transition === 'abort') f.abort();
          else f.transport.compositionStarted = true;
        });
        const pending = f.owner.capture(f.signal);
        f.flush();
        const result = await pending;
        if (result.status === 'captured') result.checkpoint.release();
        assert.deepEqual(result, {
          status: 'rejected',
          reason: transition === 'abort' ? 'cancelled' : 'composing',
        });
        assert.equal(callbacks, 1);
        assert.equal(f.timers.size, 0);
        assert.equal(f.listeners.size, 0);
        assert.deepEqual(f.counts(), {
          finished: 1,
          cancelled: 1,
          removed: 1,
          cleared: 1,
          dispatched: 1,
        });
      } finally {
        f.dispose();
      }
    });
  }
}

for (const transition of ['abort', 'composition', 'dispose', 'edit', 'failure'] as const) {
  test(`final native readiness reentry preserves ${transition} admission`, async () => {
    const f = fixture();
    try {
      f.cleanup.set('listener', () => {
        f.hooks.set('composition', () => {
          f.hooks.delete('composition');
          if (transition === 'abort') f.abort();
          else if (transition === 'composition') f.transport.compositionStarted = true;
          else if (transition === 'dispose') f.dispose();
          else if (transition === 'edit')
            f.transport.dispatch(f.transport.state.update({ changes: { from: 0, insert: 'x' } }));
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          else throw undefined;
        });
      });
      const pending = f.owner.capture(f.signal);
      f.flush();
      if (transition === 'failure') {
        let failed = false;
        await pending.catch((cause: unknown) => {
          failed = true;
          assert.equal(cause, undefined);
        });
        assert.equal(failed, true);
      } else {
        assert.deepEqual(await pending, {
          status: 'rejected',
          reason:
            transition === 'abort'
              ? 'cancelled'
              : transition === 'composition'
                ? 'composing'
                : transition === 'dispose'
                  ? 'disposed'
                  : 'superseded',
        });
      }
      assert.equal(f.timers.size, 0);
      assert.equal(f.listeners.size, 0);
    } finally {
      f.dispose();
    }
  });
}
