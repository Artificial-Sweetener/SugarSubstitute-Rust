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
/** Controlled raster providers prove finite cancellation, real reuse, queue bounds and release without browser work. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { LoraMediaPool } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-pool.ts';
import type {
  LoraMediaReference,
  LoraRaster,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';

/** Independent immutable host references intentionally distinguish identity even when URLs match. */
function reference(name: string): LoraMediaReference {
  return Object.freeze({
    status: 'ready',
    url: `https://media.invalid/${name}`,
    width: 16,
    height: 8,
  });
}
/** Count actual release rather than inferring retention from work counters. */
function raster() {
  let releases = 0;
  const value: LoraRaster = {
    width: 16,
    height: 8,
    paint() {},
    dispose() {
      releases++;
    },
  };
  return { value, releases: () => releases };
}
/** Ignored abort is deliberate; callers complete each provider independently of the pool. */
function provider() {
  const jobs: {
    readonly reference: LoraMediaReference;
    readonly signal: AbortSignal;
    readonly completion: Promise<LoraRaster | null>;
    readonly resolve: (value: LoraRaster | null) => void;
    readonly reject: (cause: unknown) => void;
  }[] = [];
  return {
    jobs,
    /** Retain a controlled host job, never a browser/view or real external service. */
    decode(reference: LoraMediaReference, signal: AbortSignal): Promise<LoraRaster | null> {
      let resolve!: (value: LoraRaster | null) => void, reject!: (cause: unknown) => void;
      const completion = new Promise<LoraRaster | null>((done, fail) => {
        resolve = done;
        reject = fail;
      });
      jobs.push({ reference, signal, completion, resolve, reject });
      return completion;
    },
  };
}

test('concurrency, current pinning and inactive eviction retain actual rasters and release old pixels once', async () => {
  const host = provider();
  const pool = new LoraMediaPool(
    host.decode,
    () => {},
    (cause) => {
      throw cause;
    },
    1,
    2,
  );
  const a = reference('a'),
    b = reference('b'),
    c = reference('c');
  pool.select([a, a, b, c]);
  assert.deepEqual(
    host.jobs.map((job) => job.reference),
    [a, b],
  );
  const first = raster(),
    second = raster(),
    third = raster();
  host.jobs[0]!.resolve(first.value);
  await host.jobs[0]!.completion;
  assert.equal(host.jobs[2]?.reference, c);
  host.jobs[1]!.resolve(second.value);
  host.jobs[2]!.resolve(third.value);
  await Promise.all([host.jobs[1]!.completion, host.jobs[2]!.completion]);
  const prepared = pool.snapshot().get(a);
  pool.select([a]);
  assert.equal(pool.snapshot().get(a), prepared);
  assert.equal(first.releases(), 0);
  assert.equal(second.releases(), 1);
  assert.equal(third.releases(), 0);
  pool.select([c]);
  const reused = pool.snapshot().get(c);
  assert.equal(reused?.status, 'ready');
  if (reused?.status === 'ready') assert.equal(reused.raster, third.value);
  assert.equal(host.jobs.length, 3);
  pool.dispose();
  pool.dispose();
  assert.deepEqual([first.releases(), second.releases(), third.releases()], [1, 1, 1]);
});

test('paused, superseded and disposed work releases ignored-abort pixels without late publication or retained observers', async () => {
  const host = provider(),
    a = reference('a'),
    b = reference('b');
  let observations = 0;
  const failures: unknown[] = [];
  const pool = new LoraMediaPool(
    host.decode,
    () => observations++,
    (cause) => failures.push(cause),
  );
  pool.select([a]);
  pool.pause();
  assert.equal(host.jobs[0]!.signal.aborted, true);
  const stale = raster();
  host.jobs[0]!.resolve(stale.value);
  await host.jobs[0]!.completion;
  assert.equal(stale.releases(), 1);
  assert.equal(observations, 0);
  pool.select([a]);
  pool.select([b]);
  const removed = raster();
  host.jobs[1]!.resolve(removed.value);
  await host.jobs[1]!.completion;
  assert.equal(removed.releases(), 1);
  assert.equal(pool.snapshot().has(a), false);
  pool.dispose();
  const late = raster();
  host.jobs[2]!.resolve(late.value);
  await host.jobs[2]!.completion;
  assert.equal(late.releases(), 1);
  assert.equal(observations, 0);
  assert.equal(failures.length, 0);
  pool.select([a]);
  assert.equal(host.jobs.length, 3);
});

test('failed media stays explicit without retry loops, current unexpected causes are observed, and replaced descriptors decode anew', async () => {
  const host = provider(),
    a = reference('same'),
    failures: unknown[] = [];
  const pool = new LoraMediaPool(
    host.decode,
    () => {},
    (cause) => failures.push(cause),
  );
  pool.select([a]);
  host.jobs[0]!.resolve(null);
  await host.jobs[0]!.completion;
  assert.equal(pool.snapshot().get(a)?.status, 'unavailable');
  pool.select([a]);
  assert.equal(host.jobs.length, 1);
  const replacement = reference('same'),
    cause = new Error('decode invariant');
  pool.select([replacement]);
  host.jobs[1]!.reject(cause);
  await host.jobs[1]!.completion.catch(() => {});
  assert.equal(pool.snapshot().get(replacement)?.status, 'unavailable');
  assert.equal(failures[0], cause);
  pool.dispose();
});

test('synchronous provider reentrancy retires the old job before current source decoding begins', async () => {
  const a = reference('a'),
    b = reference('b'),
    pixels = raster();
  const signals: AbortSignal[] = [];
  const pool: LoraMediaPool = new LoraMediaPool(
    async (input, signal) => {
      signals.push(signal);
      if (input === a) {
        pool.select([b]);
        return pixels.value;
      }
      return null;
    },
    () => {},
    (cause) => {
      throw cause;
    },
  );
  pool.select([a]);
  await Promise.resolve();
  assert.equal(signals[0]?.aborted, true);
  assert.equal(pixels.releases(), 1);
  assert.equal(pool.snapshot().has(a), false);
  assert.equal(pool.snapshot().get(b)?.status, 'unavailable');
  pool.dispose();
  assert.throws(
    () =>
      new LoraMediaPool(
        async () => null,
        () => {},
        () => {},
        -1,
      ),
    RangeError,
  );
});

test('synchronous decoder failures advance a large queue without recursive owner reentry', () => {
  let failures = 0;
  const pool = new LoraMediaPool(
    () => {
      throw new Error('Controlled synchronous decoder failure');
    },
    () => {},
    () => failures++,
  );
  pool.select(Array.from({ length: 2048 }, (_, index) => reference(String(index))));
  assert.equal(failures, 2048);
  assert.equal(
    [...pool.snapshot().values()].every((value) => value.status === 'unavailable'),
    true,
  );
  pool.dispose();
});

for (const cause of [new Error('Controlled raster release failure'), null, undefined]) {
  test(`pool disposal preserves a thrown ${cause === null ? 'null' : cause === undefined ? 'undefined' : 'Error'} while releasing later pixels and pending work`, async () => {
    const host = provider(),
      releases: string[] = [];
    let observations = 0;
    const pool = new LoraMediaPool(
      host.decode,
      () => observations++,
      () => assert.fail('Retired decode must not publish a failure'),
    );
    pool.select([reference('first'), reference('second'), reference('pending')]);
    host.jobs[0]!.resolve({
      width: 16,
      height: 8,
      /** Retirement proof acquires no canvas or painted output. */
      paint() {},
      /** A real external raster refusal must not skip the remaining owned resources. */
      dispose() {
        releases.push('first');
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      },
    });
    host.jobs[1]!.resolve({
      width: 16,
      height: 8,
      /** Retirement proof acquires no canvas or painted output. */
      paint() {},
      /** Observe independent release of the next actual prepared raster. */
      dispose() {
        releases.push('second');
      },
    });
    await Promise.all([host.jobs[0]!.completion, host.jobs[1]!.completion]);
    const before = observations;
    let failure: { readonly cause: unknown } | undefined;
    try {
      pool.dispose();
    } catch (caught) {
      failure = { cause: caught };
    }
    assert.notEqual(failure, undefined);
    assert.equal(failure!.cause, cause);
    assert.deepEqual(releases, ['first', 'second']);
    assert.equal(host.jobs[2]!.signal.aborted, true);
    assert.equal(pool.snapshot().size, 0);
    pool.dispose();
    assert.deepEqual(releases, ['first', 'second']);
    const late = raster();
    host.jobs[2]!.resolve(late.value);
    await host.jobs[2]!.completion;
    assert.equal(late.releases(), 1);
    assert.equal(observations, before);
    pool.select([reference('after-disposal')]);
    assert.equal(host.jobs.length, 3);
  });
}

test('pool retirement attempts all failing rasters and preserves every original cause in order', async () => {
  const host = provider(),
    causes = [new Error('Controlled first release failure'), null, undefined],
    releases: number[] = [];
  const pool = new LoraMediaPool(
    host.decode,
    () => {},
    () => {},
  );
  pool.select(causes.map((_, index) => reference(`failure-${index}`)));
  causes.forEach((cause, index) => {
    host.jobs[index]!.resolve({
      width: 16,
      height: 8,
      /** Retirement proof acquires no canvas or painted output. */
      paint() {},
      /** Each resource independently refuses release, including raw null and undefined values. */
      dispose() {
        releases.push(index);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      },
    });
  });
  await Promise.all(host.jobs.map((job) => job.completion));
  assert.throws(
    () => pool.dispose(),
    (failure: unknown) => {
      assert.ok(failure instanceof AggregateError);
      assert.deepEqual(failure.errors, causes);
      return true;
    },
  );
  assert.deepEqual(releases, [0, 1, 2]);
  assert.equal(pool.snapshot().size, 0);
  pool.dispose();
  assert.deepEqual(releases, [0, 1, 2]);
});
