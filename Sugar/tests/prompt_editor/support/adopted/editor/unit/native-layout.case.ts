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
/** Actual plugin registration and measurement callbacks characterize host resize invalidation and its finite native lifetime. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { nativeLayoutFixture } from './native-layout-fixture.ts';

test('host width rewrap without a CodeMirror update queues actual current content and scale publication', () => {
  const f = nativeLayoutFixture();
  try {
    f.create();
    f.flush();
    assert.equal(f.published[0]?.contentHeight, 33);
    assert.equal(f.published[0]?.viewportWidth, 700);
    f.geometry.content = 88;
    f.geometry.scale = 1.25;
    f.geometry.viewport = 88;
    f.geometry.width = 320;
    const reads = f.reads;
    f.resize();
    assert.equal(f.reads, reads);
    assert.equal(f.requests.length, 1);
    f.flush();
    assert.equal(f.published.length, 2);
    assert.equal(f.published[1]?.contentHeight, 88);
    assert.equal(f.published[1]?.scaleY, 1.25);
    assert.equal(f.published[1]?.viewportWidth, 320);
    assert.deepEqual(f.instances[0]?.targets, new Set([f.view.dom, f.view.contentDOM]));
  } finally {
    f.close();
  }
});

test('repeated native resize callbacks coalesce before one existing read and unchanged output is not republished', () => {
  const f = nativeLayoutFixture();
  try {
    f.create();
    f.resize();
    f.resize();
    assert.equal(f.requests.length, 1);
    assert.equal(f.reads, 0);
    f.flush();
    f.resize();
    f.resize();
    assert.equal(f.requests.length, 1);
    f.flush();
    assert.equal(f.published.length, 1);
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('observer to shell sizing feedback reaches stable measured output without a second publication owner', () => {
  const f = nativeLayoutFixture();
  try {
    f.geometry.viewport = 12;
    f.onPublish((value) => {
      f.geometry.viewport = value.contentHeight;
      f.resize();
    });
    f.create();
    f.flush();
    assert.equal(f.requests.length, 1);
    f.flush();
    assert.equal(f.requests.length, 1);
    f.flush();
    assert.equal(f.requests.length, 0);
    assert.equal(f.published.length, 2);
    assert.deepEqual(
      f.published.map((value) => value.viewportHeight),
      [12, 33],
    );
  } finally {
    f.close();
  }
});

test('queued native reads and late observer callbacks cannot publish after disconnect', () => {
  const f = nativeLayoutFixture();
  try {
    f.create();
    f.destroy();
    assert.equal(f.instances[0]?.disconnects, 1);
    assert.equal(f.instances[0]?.targets.size, 0);
    f.flush();
    f.lateResize();
    assert.equal(f.requests.length, 0);
    assert.equal(f.published.length, 0);
    assert.equal(f.reads, 0);
    f.destroy();
    assert.equal(f.instances[0]?.disconnects, 1);
  } finally {
    f.close();
  }
});

test('disposal between actual measurement and publication refuses the captured output', () => {
  const f = nativeLayoutFixture();
  try {
    f.create();
    const measured = f.read();
    f.destroy();
    measured.request.write(measured.value);
    f.lateResize();
    assert.equal(f.published.length, 0);
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('an absent public layout consumer acquires no native observer or measurement', () => {
  const f = nativeLayoutFixture();
  try {
    f.create(false);
    assert.equal(f.instances.length, 0);
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('native observer constructor failure remains the original raw undefined cause', () => {
  const f = nativeLayoutFixture({ construction: { cause: undefined } });
  try {
    let caught = false;
    try {
      f.create();
    } catch (cause) {
      caught = true;
      assert.equal(cause, undefined);
    }
    assert.equal(caught, true);
    assert.equal(f.instances.length, 0);
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('partial second-target observation rolls back the native observer with its raw null cause', () => {
  const f = nativeLayoutFixture({ observe: { at: 2, cause: null } });
  try {
    let caught = false;
    try {
      f.create();
    } catch (cause) {
      caught = true;
      assert.equal(cause, null);
    }
    assert.equal(caught, true);
    assert.equal(f.instances[0]?.disconnects, 1);
    assert.equal(f.instances[0]?.targets.size, 0);
    f.lateResize();
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('observer rollback preserves ordered raw primary and disconnect failures', () => {
  const f = nativeLayoutFixture({
    observe: { at: 2, cause: null },
    disconnect: { cause: undefined },
  });
  try {
    assert.throws(
      () => f.create(),
      (cause: unknown) => {
        assert.ok(cause instanceof AggregateError);
        assert.deepEqual(cause.errors, [null, undefined]);
        return true;
      },
    );
    assert.equal(f.instances[0]?.disconnects, 1);
    f.lateResize();
    assert.equal(f.requests.length, 0);
  } finally {
    f.close();
  }
});

test('failed initial scheduling retires both observed targets before exposing its raw cause', () => {
  const raw = new Error('controlled-layout-request'),
    f = nativeLayoutFixture({ request: { cause: raw } });
  try {
    assert.throws(
      () => f.create(),
      (cause) => cause === raw,
    );
    assert.equal(f.instances[0]?.disconnects, 1);
    assert.equal(f.instances[0]?.targets.size, 0);
  } finally {
    f.close();
  }
});

test('later resize scheduling failure retires the mounted observer and preserves raw cleanup causes', () => {
  const faults: {
    request?: { cause: unknown };
    disconnect?: { cause: unknown };
  } = {};
  const f = nativeLayoutFixture(faults);
  try {
    f.create();
    f.flush();
    faults.request = { cause: null };
    faults.disconnect = { cause: undefined };
    assert.throws(
      () => f.resize(),
      (cause: unknown) => {
        assert.ok(cause instanceof AggregateError);
        assert.deepEqual(cause.errors, [null, undefined]);
        return true;
      },
    );
    assert.equal(f.instances[0]?.disconnects, 1);
    assert.equal(f.instances[0]?.targets.size, 0);
    f.lateResize();
    assert.equal(f.requests.length, 0);
    assert.equal(f.published.length, 1);
    f.destroy();
    assert.equal(f.instances[0]?.disconnects, 1);
  } finally {
    f.close();
  }
});
