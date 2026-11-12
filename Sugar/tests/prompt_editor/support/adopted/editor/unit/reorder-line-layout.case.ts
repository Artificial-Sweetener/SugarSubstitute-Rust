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
/** Prepared line ownership proves actual paint/measurement reuse and finite cleanup, without browser certification. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';

/** Only native removal is faked; paint identity and immutable geometry come from the actual owner. */
function specimen(cause?: Error) {
  let destroyed = 0,
    removed = 0;
  const node = {
    remove() {
      removed++;
    },
  } as unknown as HTMLElement;
  return {
    node,
    get destroyed() {
      return destroyed;
    },
    get removed() {
      return removed;
    },
    create: () => ({
      node,
      destroy: () => {
        destroyed++;
        if (cause) throw cause;
      },
    }),
  };
}
const measured = Object.freeze({
  height: 20.375,
  rects: Object.freeze([
    {
      kind: 'chip' as const,
      index: 7,
      left: 4.125,
      right: 18.5625,
      top: 0,
      bottom: 20.375,
    },
  ]),
});

test('target revisit reuses the actual painted line and its exact measured fragments', () => {
  const owner = new ReorderLineLayout(2),
    paint = specimen(),
    first = owner.borrow('origin/font/width', paint.create);
  first.measure('native-context', measured);
  const geometry = first.geometry('native-context');
  first.release();
  assert.equal(first.geometry('native-context'), null);
  const next = owner.borrow('origin/font/width', () => {
    throw new Error('unexpected-line-reconstruction');
  });
  assert.equal(next.node, first.node);
  assert.equal(next.geometry('native-context'), geometry);
  assert.deepEqual(next.geometry('native-context'), measured);
  assert.equal(paint.destroyed, 0);
  next.release();
  owner.destroy();
  owner.destroy();
  assert.equal(paint.destroyed, 1);
});

test('width or appearance key change cannot borrow an unrelated layout; concurrent frames own distinct paint', () => {
  const owner = new ReorderLineLayout(),
    a = specimen(),
    b = specimen(),
    c = specimen();
  const first = owner.borrow('origin/240.375/font-a', a.create);
  first.measure('native-context', measured);
  const concurrent = owner.borrow('origin/240.375/font-a', b.create),
    resized = owner.borrow('origin/241/font-a', c.create);
  assert.notEqual(first.node, concurrent.node);
  assert.deepEqual(concurrent.geometry('native-context'), first.geometry('native-context'));
  assert.equal(resized.geometry('native-context'), null);
  assert.notEqual(resized.node, first.node);
  owner.destroy();
  first.release();
  concurrent.release();
  resized.release();
  assert.equal(concurrent.geometry('native-context'), null);
  assert.deepEqual([a.destroyed, b.destroyed, c.destroyed], [1, 1, 1]);
  assert.throws(() => first.measure('native-context', measured), {
    message: 'reorder-line-measurement-retired',
  });
  assert.throws(() => owner.borrow('other', a.create), {
    message: 'reorder-line-cache-retired',
  });
});

test('retained paint rejects geometry from a different native width or hard-line edge context', () => {
  const owner = new ReorderLineLayout(),
    paint = specimen();
  const first = owner.borrow('canonical-origin', paint.create);
  first.measure('240.375/partial-prefix/font-a', measured);
  const initial = first.geometry('240.375/partial-prefix/font-a');
  first.release();
  const next = owner.borrow('canonical-origin', () => {
    throw new Error('unexpected-line-reconstruction');
  });
  assert.equal(next.node, first.node);
  assert.equal(next.geometry('240.375/partial-prefix/font-a'), initial);
  assert.equal(next.geometry('241/partial-prefix/font-a'), null);
  assert.equal(next.geometry('240.375/full-line/font-a'), null);
  assert.equal(next.geometry('240.375/partial-prefix/font-b'), null);
  next.measure('241/partial-prefix/font-a', {
    height: 40.75,
    rects: [measured.rects[0]!, { ...measured.rects[0]!, top: 20.375, bottom: 40.75 }],
  });
  assert.equal(next.geometry('240.375/partial-prefix/font-a'), null);
  assert.equal(next.geometry('241/partial-prefix/font-a')!.rects.length, 2);
  next.release();
  owner.destroy();
});

test('idle eviction preserves active paint and destroys the exact oldest released line', () => {
  const owner = new ReorderLineLayout(1),
    a = specimen(),
    b = specimen(),
    held = specimen();
  const active = owner.borrow('held', held.create),
    first = owner.borrow('a', a.create);
  first.release();
  const second = owner.borrow('b', b.create);
  second.release();
  assert.deepEqual([a.destroyed, b.destroyed, held.destroyed], [1, 0, 0]);
  assert.equal(active.node, held.node);
  owner.destroy();
  assert.deepEqual([a.destroyed, b.destroyed, held.destroyed], [1, 1, 1]);
});

test('cleanup causes remain observable while every later canonical paint is retired', () => {
  const cause = new Error('controlled-line-retirement'),
    owner = new ReorderLineLayout(),
    first = specimen(cause),
    second = specimen();
  owner.borrow('a', first.create);
  owner.borrow('b', second.create);
  assert.throws(
    () => owner.destroy(),
    (error: unknown) => error === cause,
  );
  assert.deepEqual([first.destroyed, second.destroyed], [1, 1]);
  assert.deepEqual([first.removed, second.removed], [1, 1]);
});

test('reentrant retirement during creation releases that genuinely created paint without renewing the cache', () => {
  const owner = new ReorderLineLayout(),
    paint = specimen();
  assert.throws(
    () =>
      owner.borrow('origin', () => {
        owner.destroy();
        return paint.create();
      }),
    { message: 'reorder-line-cache-retired' },
  );
  assert.equal(paint.destroyed, 1);
  assert.equal(paint.removed, 1);
});

/** Each piece has an independent real cache key, actual node identity and immutable measured output. */
function framePieces(owner: ReorderLineLayout, count: number) {
  const paints = Array.from({ length: count }, () => specimen());
  const leases = paints.map((paint, index) => {
    const lease = owner.borrow(`piece-${index}`, paint.create);
    lease.measure('frame-context', measured);
    return lease;
  });
  return { paints, leases };
}

test('361-piece frame reconciliation reuses every exact node and measured geometry before idle eviction', () => {
  const owner = new ReorderLineLayout();
  const { paints, leases } = framePieces(owner, 361);
  const geometries = leases.map((lease) => lease.geometry('frame-context'));
  const next: typeof leases = [];
  owner.reconcile(() => {
    for (const lease of leases) lease.release();
    for (let index = 0; index < leases.length; index++) {
      const lease = owner.borrow(`piece-${index}`, () => {
        throw new Error('frame-reconstructed-retained-paint');
      });
      assert.equal(lease.node, paints[index]!.node);
      assert.equal(lease.geometry('frame-context'), geometries[index]);
      next.push(lease);
    }
  });
  assert.equal(
    paints.reduce((sum, paint) => sum + paint.destroyed, 0),
    0,
  );
  owner.reconcile(() => {
    for (const lease of next) lease.release();
  });
  assert.equal(
    paints.reduce((sum, paint) => sum + paint.destroyed, 0),
    105,
  );
  owner.destroy();
  assert.ok(paints.every((paint) => paint.destroyed === 1));
});

test('nested replacement does not trim until the outer frame has finished reborrowing', () => {
  const owner = new ReorderLineLayout(1);
  const { paints, leases } = framePieces(owner, 3);
  owner.reconcile(() => {
    leases[0]!.release();
    owner.reconcile(() => {
      leases[1]!.release();
      leases[2]!.release();
    });
    assert.ok(paints.every((paint) => paint.destroyed === 0));
    for (let i = 0; i < paints.length; i++)
      assert.equal(
        owner.borrow(`piece-${i}`, () => {
          throw new Error('nested-reconstruction');
        }).node,
        paints[i]!.node,
      );
  });
  owner.destroy();
  assert.ok(paints.every((paint) => paint.destroyed === 1));
});
for (const primary of [null, undefined, new Error('frame-action')] as const)
  test(`action failure remains first while idle trimming still releases every over-budget piece: ${String(primary)}`, () => {
    const owner = new ReorderLineLayout(1),
      cleanup = new Error('trim-cleanup');
    const node = specimen();
    const first = owner.borrow('first', () => ({
      node: node.node,
      destroy: () => {
        throw cleanup;
      },
    }));
    const lastPaint = specimen(),
      last = owner.borrow('last', lastPaint.create);
    assert.throws(
      () =>
        owner.reconcile(() => {
          first.release();
          last.release();
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        }),
      (cause: unknown) => {
        assert.ok(cause instanceof AggregateError);
        assert.equal(cause.errors[0], primary);
        const trim: unknown = cause.errors[1];
        assert.ok(trim instanceof AggregateError);
        assert.deepEqual(trim.errors, [cleanup]);
        return true;
      },
    );
    const retained = owner.borrow('last', () => {
      throw new Error('idle-tail-lost');
    });
    assert.equal(retained.node, lastPaint.node);
    owner.destroy();
  });
for (const primary of [null, undefined] as const)
  test(`single raw action cause survives successful scope cleanup: ${String(primary)}`, () => {
    const owner = new ReorderLineLayout(1);
    let caught = false;
    try {
      owner.reconcile(() => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw primary;
      });
    } catch (cause) {
      caught = true;
      assert.equal(cause, primary);
    }
    assert.equal(caught, true);
    owner.destroy();
  });

test('disposal during nested reconciliation retires all paint once and forbids later admission', () => {
  const owner = new ReorderLineLayout(1),
    { paints, leases } = framePieces(owner, 3);
  owner.reconcile(() => {
    leases[0]!.release();
    owner.reconcile(() => owner.destroy());
    for (const lease of leases) lease.release();
  });
  owner.destroy();
  assert.ok(paints.every((paint) => paint.destroyed === 1));
  let invoked = false;
  assert.throws(
    () =>
      owner.reconcile(() => {
        invoked = true;
      }),
    { message: 'reorder-line-cache-retired' },
  );
  assert.equal(invoked, false);
});

test('eviction cleanup may synchronously reconcile a newer borrowed line without retiring it', () => {
  const owner = new ReorderLineLayout(1),
    fresh = specimen(),
    tail = specimen(),
    old = specimen();
  let newer: ReturnType<ReorderLineLayout['borrow']> | null = null;
  const first = owner.borrow('old', () => ({
    node: old.node,
    destroy: () => {
      owner.reconcile(() => {
        newer = owner.borrow('new', fresh.create);
      });
    },
  }));
  const last = owner.borrow('tail', tail.create);
  owner.reconcile(() => {
    first.release();
    last.release();
  });
  assert.ok(newer);
  assert.equal(fresh.destroyed, 0);
  assert.equal(tail.destroyed, 0);
  owner.destroy();
  assert.equal(fresh.destroyed, 1);
  assert.equal(tail.destroyed, 1);
});
