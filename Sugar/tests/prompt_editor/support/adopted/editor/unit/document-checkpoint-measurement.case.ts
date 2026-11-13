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
/** Native capture respects supported measurement phases and retires queued continuations before mutation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { fixture } from './document-checkpoint-lifetime-fixture.ts';
import { checkpointFixture } from '../transaction/document-checkpoint-fixture.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';

test('capture finishes input and dispatches only after the measurement write callback returns', async () => {
  const f = fixture();
  let measuring = false;
  /** The external measurement seam enforces the pinned CodeMirror restriction without accessing native private state. */
  const requireOutsideWrite = () => {
    if (measuring)
      throw new Error('Calls to EditorView.update are not allowed while an update is in progress');
  };
  f.hooks.set('finish', requireOutsideWrite);
  f.hooks.set('dispatch', requireOutsideWrite);
  try {
    const pending = f.owner.capture();
    measuring = true;
    f.flush();
    measuring = false;
    const during = f.counts();
    const result = await pending;
    assert.equal(during.finished, 0);
    assert.equal(during.dispatched, 0);
    assert.equal(result.status, 'captured');
    if (result.status === 'captured') result.checkpoint.release();
    assert.equal(f.counts().finished, 1);
    assert.equal(f.counts().dispatched, 1);
  } finally {
    measuring = false;
    f.dispose();
  }
});

for (const transition of [
  'abort',
  'dispose',
  'supersede',
  'deadline',
  'composition',
  'source',
  'selection',
] as const) {
  test(`queued capture remains inert after ${transition}`, async () => {
    const f = fixture();
    try {
      const pending = f.owner.capture(f.signal);
      f.flush();
      if (transition === 'abort') f.abort();
      else if (transition === 'dispose') f.dispose();
      else if (transition === 'supersede') f.owner.supersede();
      else if (transition === 'deadline') f.deadline();
      else if (transition === 'composition') f.transport.compositionStarted = true;
      else
        f.transport.dispatch(
          f.transport.state.update(
            transition === 'source'
              ? { changes: { from: 0, insert: 'x' } }
              : { selection: EditorSelection.cursor(1) },
          ),
        );
      const result = await pending;
      if (result.status === 'captured') result.checkpoint.release();
      assert.deepEqual(result, {
        status: 'rejected',
        reason:
          transition === 'abort'
            ? 'cancelled'
            : transition === 'dispose'
              ? 'disposed'
              : transition === 'deadline'
                ? 'deadline'
                : transition === 'composition'
                  ? 'composing'
                  : 'superseded',
      });
      assert.equal(f.counts().finished, 0);
      assert.equal(f.timers.size, 0);
      assert.equal(f.listeners.size, 0);
    } finally {
      f.dispose();
    }
  });
}

test('queued actual native host capture rejects command-context replacement', async () => {
  const f = checkpointFixture();
  try {
    const pending = f.updates.captureDocument();
    f.flush();
    f.transport.dispatch({ effects: invalidateCommandContext.of(null) });
    assert.deepEqual(await pending, {
      status: 'rejected',
      reason: 'superseded',
    });
    assert.equal(f.counts().finished, 0);
  } finally {
    f.dispose();
  }
});

for (const cleanup of [false, true]) {
  test(`microtask admission preserves thrown undefined with cleanup failures ${cleanup}`, async () => {
    const f = fixture(),
      original = globalThis.queueMicrotask;
    const secondary = new Error('controlled cleanup');
    if (cleanup) {
      f.cleanup.set('input', () => {
        throw secondary;
      });
      f.cleanup.set('timer', () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw undefined;
      });
      f.cleanup.set('listener', () => {
        throw secondary;
      });
    }
    try {
      const pending = f.owner.capture(f.signal);
      try {
        globalThis.queueMicrotask = () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw undefined;
        };
        f.flush();
      } finally {
        globalThis.queueMicrotask = original;
      }
      let rejected = false;
      await pending.catch((cause: unknown) => {
        rejected = true;
        if (cleanup) {
          assert(cause instanceof AggregateError);
          assert.deepEqual(cause.errors, [undefined, secondary, undefined, secondary]);
        } else assert.equal(cause, undefined);
      });
      assert.equal(rejected, true);
      assert.equal(f.counts().cancelled, 1);
      assert.equal(f.counts().cleared, 1);
      assert.equal(f.counts().removed, 1);
    } finally {
      globalThis.queueMicrotask = original;
      f.cleanup.clear();
      f.dispose();
    }
  });
}
