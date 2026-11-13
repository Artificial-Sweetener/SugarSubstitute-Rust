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
/** Actual native owner controls cover finite cancellation, cleanup and publication reentry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import type { NativeDocumentCheckpoints } from '../../../../../../frontend/sugarprompt/editor/src/native-document-checkpoints.ts';
import { fixture } from './document-checkpoint-lifetime-fixture.ts';
test('successful capture releases every owned resource and authenticates only its frozen document handle', async () => {
  const f = fixture();
  const pending = f.owner.capture(f.signal);
  f.flush();
  const result = await pending;
  assert.equal(result.status, 'captured');
  if (result.status !== 'captured') return assert.fail('CAPTURE_REQUIRED');
  assert.equal(result.checkpoint.source === 'current', true);
  assert.ok(Object.isFrozen(result.checkpoint));
  assert.equal(f.timers.size, 0);
  assert.equal(f.listeners.size, 0);
  assert.deepEqual(f.counts(), {
    finished: 1,
    cancelled: 1,
    removed: 1,
    cleared: 1,
    dispatched: 1,
  });
  result.checkpoint.release();
  assert.equal(f.owner.inspect(result.checkpoint), 'released-checkpoint');
  f.dispose();
});

for (const terminal of ['abort', 'deadline', 'supersede', 'dispose'] as const) {
  test(`${terminal} settles once and makes queued measurement work inert`, async () => {
    const f = fixture();
    const pending = f.owner.capture(f.signal);
    if (terminal === 'abort') f.abort();
    else if (terminal === 'deadline') f.deadline();
    else if (terminal === 'supersede') f.owner.supersede();
    else f.dispose();
    assert.deepEqual(await pending, {
      status: 'rejected',
      reason:
        terminal === 'abort'
          ? 'cancelled'
          : terminal === 'supersede'
            ? 'superseded'
            : terminal === 'dispose'
              ? 'disposed'
              : 'deadline',
    });
    f.flush();
    f.deadline();
    f.abort();
    assert.deepEqual(f.counts(), {
      finished: 0,
      cancelled: 1,
      removed: 1,
      cleared: 1,
      dispatched: 0,
    });
    assert.equal(f.timers.size, 0);
    assert.equal(f.listeners.size, 0);
    f.dispose();
  });
}

for (const stage of [
  'reserve',
  'allocate',
  'subscribe',
  'measure',
  'finish',
  'dispatch',
] as const) {
  test(`${stage} thrown undefined remains an observable primary while owned releases are attempted`, async () => {
    const f = fixture();
    f.hooks.set(stage, () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw undefined;
    });
    const pending = f.owner.capture(f.signal);
    f.flush();
    await assert.rejects(pending, (cause: unknown) => cause === undefined);
    assert.equal(f.listeners.size, 0);
    if (stage !== 'allocate') assert.equal(f.timers.size, 0);
    // A failed allocation did not return its external handle; the controlled allocator retires its own unreturned resource.
    f.timers.clear();
    f.dispose();
  });
}

for (const mask of [0, 1, 2, 3, 4, 5, 6, 7]) {
  test(`cleanup mask ${mask} retains exact ordered causes and attempts every independent release`, async () => {
    const f = fixture();
    const primary = undefined,
      input = Symbol(),
      timer = undefined,
      listener = Symbol();
    f.hooks.set('finish', () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw primary;
    });
    const expected: unknown[] = [primary];
    if (mask & 1) {
      f.cleanup.set('input', () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw input;
      });
      expected.push(input);
    }
    if (mask & 2) {
      f.cleanup.set('timer', () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw timer;
      });
      expected.push(timer);
    }
    if (mask & 4) {
      f.cleanup.set('listener', () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw listener;
      });
      expected.push(listener);
    }
    const pending = f.owner.capture(f.signal);
    f.flush();
    await assert.rejects(pending, (cause: unknown) => {
      if (expected.length === 1) assert.equal(cause, primary);
      else {
        assert.ok(cause instanceof AggregateError);
        assert.deepEqual(cause.errors, expected);
      }
      return true;
    });
    assert.deepEqual(f.counts(), {
      finished: 1,
      cancelled: 1,
      removed: 1,
      cleared: 1,
      dispatched: 0,
    });
    const before = f.counts();
    f.deadline();
    f.abort();
    f.flush();
    f.dispose();
    assert.deepEqual(f.counts(), before);
    // Failed external removals remain observable; retained fake callbacks are terminal and inert.
    f.timers.clear();
    f.listeners.clear();
  });
}

for (const stage of ['reserve', 'allocate', 'subscribe', 'measure'] as const) {
  test(`${stage} synchronous teardown releases even resources returned after reentry`, async () => {
    const f = fixture();
    f.hooks.set(stage, () => f.dispose());
    const pending = f.owner.capture(f.signal);
    assert.deepEqual(await pending, { status: 'rejected', reason: 'disposed' });
    f.flush();
    assert.equal(f.timers.size, 0);
    assert.equal(f.listeners.size, 0);
    assert.equal(f.counts().cancelled, 1);
    assert.equal(f.counts().finished, 0);
  });
}

test('a later capture supersedes the previous request without cancelling its new deadline or reservation', async () => {
  const f = fixture();
  const first = f.owner.capture(f.signal);
  const second = f.owner.capture(f.signal);
  assert.deepEqual(await first, { status: 'rejected', reason: 'superseded' });
  f.flush();
  const result = await second;
  assert.equal(result.status, 'captured');
  if (result.status === 'captured') result.checkpoint.release();
  assert.equal(f.timers.size, 0);
  assert.equal(f.listeners.size, 0);
  assert.deepEqual(f.counts(), {
    finished: 1,
    cancelled: 2,
    removed: 2,
    cleared: 2,
    dispatched: 1,
  });
  f.dispose();
});

test('a source change between measurement read and write rejects before input settlement', async () => {
  const f = fixture();
  const pending = f.owner.capture(f.signal);
  const request = f.measurements.shift();
  assert.ok(request);
  const view = f.transport as unknown as EditorView;
  const ready = request.read(view);
  assert.equal(ready, true);
  f.transport.dispatch(f.transport.state.update({ changes: { from: 0, insert: 'new' } }));
  request.write?.(ready, view);
  assert.deepEqual(await pending, { status: 'rejected', reason: 'superseded' });
  assert.equal(f.counts().finished, 0);
  assert.equal(f.timers.size, 0);
  assert.equal(f.listeners.size, 0);
  f.dispose();
});

test('capture rejects composition and already cancelled or disposed calls without allocations', async () => {
  const f = fixture();
  f.transport.compositionStarted = true;
  assert.deepEqual(await f.owner.capture(), {
    status: 'rejected',
    reason: 'composing',
  });
  f.transport.compositionStarted = false;
  f.abort();
  assert.deepEqual(await f.owner.capture(f.signal), {
    status: 'rejected',
    reason: 'cancelled',
  });
  f.dispose();
  assert.deepEqual(await f.owner.capture(), {
    status: 'rejected',
    reason: 'disposed',
  });
  assert.deepEqual(f.counts(), {
    finished: 0,
    cancelled: 0,
    removed: 0,
    cleared: 0,
    dispatched: 0,
  });
});

test('composition beginning after a measurement read rejects before input settlement', async () => {
  const f = fixture(),
    pending = f.owner.capture(f.signal);
  const request = f.measurements.shift();
  assert.ok(request);
  const view = f.transport as unknown as EditorView,
    ready = request.read(view);
  f.transport.compositionStarted = true;
  request.write?.(ready, view);
  assert.deepEqual(await pending, { status: 'rejected', reason: 'composing' });
  assert.equal(f.counts().finished, 0);
  assert.equal(f.timers.size, 0);
  assert.equal(f.listeners.size, 0);
  f.dispose();
});

for (const stage of ['liveness', 'view'] as const) {
  for (const phase of ['read', 'write'] as const) {
    test(`${phase} ${stage} failure retains thrown undefined and releases the capture`, async () => {
      const f = fixture(),
        pending = f.owner.capture(f.signal);
      const request = f.measurements.shift();
      assert.ok(request);
      const view = f.transport as unknown as EditorView;
      const ready = phase === 'write' ? request.read(view) : undefined;
      f.hooks.set(stage, () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw undefined;
      });
      if (phase === 'read') request.read(view);
      else request.write?.(ready, view);
      await assert.rejects(pending, (cause: unknown) => cause === undefined);
      assert.equal(f.timers.size, 0);
      assert.equal(f.listeners.size, 0);
      f.hooks.clear();
      f.dispose();
    });
  }
}

test('preflight failure returns a rejected promise with the original undefined cause', async () => {
  const f = fixture();
  f.hooks.set('liveness', () => {
    // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
    throw undefined;
  });
  await assert.rejects(f.owner.capture(), (cause: unknown) => cause === undefined);
  assert.equal(f.timers.size, 0);
  assert.equal(f.listeners.size, 0);
  f.hooks.clear();
  f.dispose();
});

for (const stage of ['reserve', 'allocate', 'subscribe'] as const) {
  test(`${stage} newer capture reentry preserves the new request and retires the old returned resources`, async () => {
    const f = fixture();
    let newer: ReturnType<NativeDocumentCheckpoints['capture']> | undefined;
    f.hooks.set(stage, () => {
      f.hooks.delete(stage);
      newer = f.owner.capture(f.signal);
    });
    const first = f.owner.capture(f.signal);
    assert.deepEqual(await first, { status: 'rejected', reason: 'superseded' });
    assert.ok(newer);
    f.flush();
    const result = await newer;
    assert.equal(result.status, 'captured');
    if (result.status === 'captured') result.checkpoint.release();
    assert.equal(f.counts().cancelled, 2);
    assert.equal(f.counts().finished, 1);
    assert.equal(f.timers.size, 0);
    assert.equal(f.listeners.size, 0);
    f.dispose();
  });
}

for (const stage of ['input', 'timer', 'listener'] as const) {
  test(`capture publication rejects disposal during ${stage} cleanup`, async () => {
    const f = fixture();
    f.cleanup.set(stage, () => f.dispose());
    const pending = f.owner.capture(f.signal);
    f.flush();
    assert.deepEqual(await pending, { status: 'rejected', reason: 'disposed' });
    assert.equal(f.timers.size, 0);
    assert.equal(f.listeners.size, 0);
  });
  test(`capture publication rejects supersession during ${stage} cleanup without retiring newer work`, async () => {
    const f = fixture();
    let newer: ReturnType<NativeDocumentCheckpoints['capture']> | undefined;
    f.cleanup.set(stage, () => {
      f.cleanup.delete(stage);
      newer = f.owner.capture(f.signal);
    });
    const pending = f.owner.capture(f.signal);
    f.flush();
    assert.deepEqual(await pending, {
      status: 'rejected',
      reason: 'superseded',
    });
    assert.ok(newer);
    f.flush();
    const result = await newer;
    assert.equal(result.status, 'captured');
    if (result.status === 'captured') result.checkpoint.release();
    assert.equal(f.timers.size, 0);
    assert.equal(f.listeners.size, 0);
    f.dispose();
  });
}
