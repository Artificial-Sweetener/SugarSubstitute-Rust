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
/** Controlled fake host requests prove serialized updates, failure observability and finite disposal. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  PreferenceSession,
  defaultPreferences,
  type FeaturePreferences,
  type PreferenceAdapter,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import type {
  PreferenceRead,
  PreferenceWrite,
} from '../../../../../../frontend/sugarprompt/services/src/features/contracts.ts';

/** Manually completed boundary; test progression follows semantic request arrival, never sleeps. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test('load is coalesced and simultaneous independent updates persist serially without losing values', async () => {
  const firstWrite = deferred<PreferenceWrite>(),
    entered = deferred<void>();
  const saved: FeaturePreferences[] = [];
  let reads = 0;
  const adapter: PreferenceAdapter = {
    /** Count real host reads, returning an older partial decoded snapshot. */
    async read() {
      reads++;
      return { status: 'ready', value: { allowed: { emphasis: false } } };
    },
    /** Delay exactly the first write to prove that the next write waits for it. */
    async write(value) {
      saved.push(value);
      if (saved.length === 1) {
        entered.resolve();
        return firstWrite.promise;
      }
      return { status: 'saved' };
    },
  };
  const session = new PreferenceSession(adapter);
  assert.equal(session.load(), session.load());
  await session.load();
  const first = session.update({ wheelMode: 'focus_required' });
  const second = session.update({ resolveOnGeneration: false });
  await entered.promise;
  assert.equal(saved.length, 1);
  assert.equal(session.snapshot().preferences.wheelMode, 'hover_dwell');
  firstWrite.resolve({ status: 'saved' });
  assert.equal((await first).status, 'saved');
  assert.equal((await second).status, 'saved');
  assert.equal(reads, 1);
  assert.equal(saved[1]?.wheelMode, 'focus_required');
  assert.equal(saved[1]?.resolveOnGeneration, false);
  assert.equal(saved[1]?.allowed.emphasis, false);
  session.dispose();
});

test('invalid decoded data preserves host bytes; unavailable writes preserve accepted preferences and may recover', async () => {
  let writes = 0;
  const invalid = new PreferenceSession({
    async read() {
      return { status: 'ready', value: { allowed: { emphasis: 'false' } } };
    },
    async write() {
      writes++;
      return { status: 'saved' };
    },
  });
  assert.equal((await invalid.load()).status, 'invalid');
  assert.equal((await invalid.update({ resolveOnGeneration: false })).status, 'invalid');
  assert.equal(writes, 0);
  const session = new PreferenceSession({
    async read() {
      return { status: 'ready', value: null };
    },
    async write() {
      writes++;
      return { status: writes === 1 ? 'unavailable' : 'saved' };
    },
  });
  await session.load();
  assert.equal((await session.update({ wheelMode: 'focus_required' })).status, 'unavailable');
  assert.equal(session.snapshot().preferences, defaultPreferences);
  assert.equal((await session.update({ resolveOnGeneration: false })).status, 'saved');
  assert.equal(session.snapshot().preferences.wheelMode, 'hover_dwell');
  assert.equal(session.snapshot().persistence, 'ready');
  invalid.dispose();
  session.dispose();
});

test('unexpected host failures reject observably and subsequent valid writes can proceed', async () => {
  const failure = new Error('synthetic host failure');
  let writes = 0;
  const session = new PreferenceSession({
    async read() {
      return { status: 'ready', value: null };
    },
    async write() {
      if (++writes === 1) throw failure;
      return { status: 'saved' };
    },
  });
  await assert.rejects(
    session.update({ wheelMode: 'focus_required' }),
    (error: unknown) => error === failure,
  );
  assert.equal(session.snapshot().preferences, defaultPreferences);
  assert.equal((await session.update({ resolveOnGeneration: false })).status, 'saved');
  session.dispose();
});

test('disposal aborts in-flight storage, settles queued work and prevents late publication', async () => {
  const boundary = deferred<PreferenceRead>(),
    entered = deferred<void>();
  let signal: AbortSignal | undefined,
    publications = 0,
    writes = 0;
  const session = new PreferenceSession(
    {
      read(cancellation) {
        signal = cancellation;
        entered.resolve();
        cancellation.addEventListener('abort', () => boundary.resolve({ status: 'unavailable' }), {
          once: true,
        });
        return boundary.promise;
      },
      async write() {
        writes++;
        return { status: 'saved' };
      },
    },
    () => {
      publications++;
    },
  );
  const load = session.load(),
    update = session.update({ resolveOnGeneration: false });
  await entered.promise;
  session.dispose();
  assert.equal(signal?.aborted, true);
  assert.equal((await load).status, 'disposed');
  assert.equal((await update).status, 'disposed');
  assert.equal(publications, 0);
  assert.equal(writes, 0);
});

test('absent host persistence remains explicitly unavailable and never claims saved settings', async () => {
  const session = new PreferenceSession(undefined);
  assert.equal((await session.load()).status, 'unavailable');
  assert.equal((await session.update({ resolveOnGeneration: false })).status, 'unavailable');
  session.dispose();
});

test('noncooperative host completion cannot retain the service queue or publish after disposed writes', async () => {
  const write = deferred<PreferenceWrite>(),
    entered = deferred<void>();
  let publications = 0;
  const session = new PreferenceSession(
    {
      async read() {
        return { status: 'ready', value: null };
      },
      write() {
        entered.resolve();
        return write.promise;
      },
    },
    () => {
      publications++;
    },
  );
  await session.load();
  const first = session.update({ wheelMode: 'focus_required' });
  const queued = session.update({ resolveOnGeneration: false });
  await entered.promise;
  session.dispose();
  assert.equal((await first).status, 'disposed');
  assert.equal((await queued).status, 'disposed');
  assert.equal(publications, 1);
  write.resolve({ status: 'saved' });
  await write.promise;
  assert.equal(session.snapshot().preferences, defaultPreferences);
  assert.equal(publications, 1);
});

test('malformed host outcomes remain observable and cannot claim a successful write', async () => {
  const session = new PreferenceSession({
    /** A conforming decoded first-use response enables the write boundary. */
    async read() {
      return { status: 'ready', value: null };
    },
    /** Deliberately falsify the host's declared type to prove runtime outcome validation. */
    async write() {
      return { status: 'incorrect' } as unknown as PreferenceWrite;
    },
  });
  await assert.rejects(session.update({ resolveOnGeneration: false }), TypeError);
  assert.equal(session.snapshot().preferences, defaultPreferences);
  assert.equal(session.snapshot().persistence, 'failed');
  session.dispose();
});
