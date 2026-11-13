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
/** Actual shared-parent lifecycle consumes native facet membership and rejects stale or canceled publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { tooltipParentFixture } from './tooltip-parent-fixture.ts';

/** Catch by count so thrown null and undefined remain observable raw values. */
function caught(action: () => void): unknown {
  const failures: unknown[] = [];
  try {
    action();
  } catch (cause) {
    failures.push(cause);
  }
  assert.equal(failures.length, 1);
  return failures[0];
}

test('host ancestry and one shared manual portal remain inert until actual non-null tooltip membership', () => {
  const f = tooltipParentFixture();
  try {
    assert.equal(f.host.children[0], f.root);
    assert.equal(f.root.popover, 'manual');
    assert.equal(f.root.listeners.get('toggle')?.size, 1);
    assert.equal(f.root.attributes.get('role'), 'region');
    assert.equal(f.root.children.length, 1);
    assert.match(
      f.root.children[0]!.textContent,
      /::backdrop\{background:transparent;pointer-events:none\}/u,
    );
    assert.equal(f.root.style.pointerEvents, 'none');
    const view = f.bind();
    assert.deepEqual([f.root.shows, f.root.hides, f.root.checks], [0, 0, 0]);
    view.update(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open], [1, 0, true]);
    const checks = f.root.checks;
    view.update(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.checks], [1, 0, checks]);
    view.update(false);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open], [1, 1, false]);
    view.lease.destroy();
    assert.equal(f.root.removals, 0);
  } finally {
    f.destroy();
  }
  assert.equal(f.root.removals, 1);
  assert.equal(f.root.listeners.size, 0);
});

test("new native-view acquisition invalidates old callbacks without closing the replacement's active publication", () => {
  const f = tooltipParentFixture();
  try {
    const previous = f.bind(true),
      current = f.bind(true);
    previous.rejectReads();
    previous.lease.destroy();
    previous.update(false);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open], [1, 0, true]);
    current.lease.destroy();
    assert.deepEqual([f.root.hides, f.root.open], [1, false]);
  } finally {
    f.destroy();
  }
});

test('reentrant replacement with no tooltip reconciles once after the native show algorithm and old release is inert', () => {
  const f = tooltipParentFixture();
  try {
    const previous = f.bind();
    f.root.beforeShow = () => {
      f.root.beforeShow = null;
      f.bind(false);
      previous.lease.destroy();
    };
    previous.update(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open, f.jobs.length], [1, 0, true, 1]);
    f.flush();
    assert.deepEqual([f.root.hides, f.root.open, f.jobs.length], [1, false, 0]);
  } finally {
    f.destroy();
  }
});

test('queued reentrant publication cannot reopen a disposed host', () => {
  const f = tooltipParentFixture();
  try {
    const previous = f.bind();
    f.root.beforeShow = () => {
      f.root.beforeShow = null;
      f.bind(false);
    };
    previous.update(true);
    f.owner.dispose();
    const operations = [f.root.shows, f.root.hides, f.root.removals];
    f.flush();
    assert.deepEqual([f.root.shows, f.root.hides, f.root.removals], operations);
    assert.equal(f.root.open, false);
  } finally {
    f.destroy();
  }
});

test('cancelled current show fails closed and removes the host instead of pretending to publish', () => {
  const f = tooltipParentFixture();
  try {
    f.root.cancel = true;
    const view = f.bind();
    const failure = caught(() => view.update(true));
    assert.ok(failure instanceof Error);
    assert.equal(failure.message, 'NATIVE_TOOLTIP_PUBLICATION_CANCELLED');
    assert.deepEqual([f.root.open, f.root.removals], [false, 1]);
  } finally {
    f.destroy();
  }
});

for (const primary of [null, undefined])
  test(`native show retains raw ${String(primary)} while independent removal is attempted`, () => {
    const f = tooltipParentFixture();
    try {
      const view = f.bind();
      f.root.showFailure = { cause: primary };
      assert.equal(
        caught(() => view.update(true)),
        primary,
      );
      assert.equal(f.root.removals, 1);
    } finally {
      f.destroy();
    }
  });

test('native hide failure and independent removal failure retain ordered raw causes on disposal', () => {
  const f = tooltipParentFixture();
  try {
    f.bind(true);
    f.root.hideFailure = { cause: null };
    f.root.removeFailure = { cause: undefined };
    const failure = caught(() => f.owner.dispose());
    assert.ok(failure instanceof AggregateError);
    assert.deepEqual(failure.errors, [null, undefined]);
    assert.equal(f.root.removals, 1);
  } finally {
    f.destroy();
  }
});

test('partial host acquisition preserves primary and independent removal failures', () => {
  const failure = caught(() =>
    tooltipParentFixture({
      mountFailure: { cause: null },
      removeFailure: { cause: undefined },
    }),
  );
  assert.ok(failure instanceof AggregateError);
  assert.deepEqual(failure.errors, [null, undefined]);
});

test('reentrant activation during hide is reconciled for the new lease without nested native operations', () => {
  const f = tooltipParentFixture();
  try {
    const previous = f.bind(true);
    f.root.beforeHide = () => {
      f.root.beforeHide = null;
      f.bind(true);
    };
    previous.update(false);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open, f.jobs.length], [1, 1, false, 1]);
    previous.lease.destroy();
    f.flush();
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open, f.jobs.length], [2, 1, true, 0]);
  } finally {
    f.destroy();
  }
});

test('disposal inside show retires connected DOM and retains the native disconnected-operation failure', () => {
  const f = tooltipParentFixture();
  try {
    const view = f.bind();
    f.root.beforeShow = () => {
      f.root.beforeShow = null;
      f.owner.dispose();
    };
    const failure = caught(() => view.update(true));
    assert.ok(failure instanceof Error);
    assert.equal(failure.message, 'DISCONNECTED_POPOVER_BOUNDARY');
    assert.deepEqual([f.root.open, f.root.removals, f.jobs.length], [false, 1, 0]);
  } finally {
    f.destroy();
  }
});

test('failed reentrant scheduling retains raw cause and retires the already-open native portal', () => {
  const f = tooltipParentFixture();
  try {
    const view = f.bind();
    f.root.beforeShow = () => {
      f.root.beforeShow = null;
      f.bind(false);
    };
    globalThis.queueMicrotask = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw null;
    };
    assert.equal(
      caught(() => view.update(true)),
      null,
    );
    assert.deepEqual([f.root.open, f.root.hides, f.root.removals], [false, 1, 1]);
  } finally {
    f.destroy();
  }
});

test('detached activation waits for a current update and native detach toggle cannot reopen disconnected DOM', () => {
  const f = tooltipParentFixture();
  try {
    f.root.connected = false;
    const view = f.bind(true);
    assert.deepEqual([f.root.shows, f.root.open], [0, false]);
    f.root.connected = true;
    view.update(true);
    assert.deepEqual([f.root.shows, f.root.open], [1, true]);
    f.root.connected = false;
    f.root.open = false;
    f.root.dispatchEvent(new Event('toggle'));
    assert.deepEqual([f.root.shows, f.root.open], [1, false]);
    f.root.connected = true;
    view.update(true);
    assert.deepEqual([f.root.shows, f.root.open], [2, true]);
    f.owner.dispose();
    const checks = f.root.checks;
    assert.equal(f.root.listeners.size, 0);
    f.root.dispatchEvent(new Event('toggle'));
    assert.equal(f.root.checks, checks);
  } finally {
    f.destroy();
  }
});

test('an old facet getter cannot replace the new inactive lease with its stale active membership', () => {
  const f = tooltipParentFixture();
  try {
    const previous = f.bind(true, () => {
      f.bind(false);
    });
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open], [0, 0, false]);
    previous.lease.destroy();
    previous.rejectReads();
    previous.update(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open], [0, 0, false]);
  } finally {
    f.destroy();
  }
});

test('disposal during a facet getter cannot publish returned stale membership', () => {
  const f = tooltipParentFixture();
  try {
    f.bind(true, () => {
      f.owner.dispose();
    });
    assert.deepEqual(
      [f.root.shows, f.root.hides, f.root.removals, f.root.listeners.size],
      [0, 0, 1, 0],
    );
  } finally {
    f.destroy();
  }
});

test('a connectivity getter that disposes the owner cannot enter a native popover operation', () => {
  const f = tooltipParentFixture();
  try {
    f.root.beforeConnected = () => {
      f.owner.dispose();
    };
    f.bind(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.removals, f.jobs.length], [0, 0, 1, 0]);
  } finally {
    f.destroy();
  }
});

test('a connectivity getter that replaces the live lease defers only its current inactive membership', () => {
  const f = tooltipParentFixture();
  try {
    f.root.beforeConnected = () => {
      f.bind(false);
    };
    f.bind(true);
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open, f.jobs.length], [0, 0, false, 1]);
    f.flush();
    assert.deepEqual([f.root.shows, f.root.hides, f.root.open, f.jobs.length], [0, 0, false, 0]);
  } finally {
    f.destroy();
  }
});
