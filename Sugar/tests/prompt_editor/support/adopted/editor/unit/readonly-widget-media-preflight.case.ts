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
/** Whole-mount preflight consumes the same finite media owner without native geometry, source inspection or a second aggregate allowance. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import type { LoraRaster } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Prepared raster identities and literal native constructor output are independent external boundaries. */
function batch(count: number, width: number, height: number, current: () => boolean = () => true) {
  const f = motionPaintFixture(),
    media = new ReadonlyWidgetMedia(current),
    painted: HTMLCanvasElement[] = [],
    roots = Array.from({ length: count }, () => f.view.dom.ownerDocument.createElement('span'));
  let disposals = 0;
  const raster: LoraRaster = {
    width,
    height,
    /** Observe the actual duplicate buffer receiving the already-prepared raster identity. */
    paint(canvas) {
      assert.equal(this, raster);
      painted.push(canvas);
    },
    /** Prepared pixels remain borrowed under their original preparation owner. */
    dispose() {
      disposals++;
    },
  };
  for (const root of roots) {
    f.view.contentDOM.append(root);
    media.defer(root, raster);
  }
  return { f, media, roots, raster, painted, disposals: () => disposals };
}

test('45 default-decoder-sized borrowed rasters preflight once and materialize bounded duplicate canvases under the 4MP aggregate', () => {
  let currentCalls = 0;
  const f = batch(45, 256, 256, () => {
    currentCalls++;
    return true;
  });
  try {
    for (const root of f.roots) {
      root.contains = () => {
        assert.fail('whole-preflight-native-subtree-inspection');
      };
      root.getClientRects = () => {
        assert.fail('whole-preflight-native-geometry');
      };
    }
    const allocations = f.f.allocations,
      plan = f.media.preflightAll();
    assert.equal(plan.kind, 'ready');
    assert.equal(f.f.allocations, allocations);
    assert.equal(currentCalls, 0);
    if (plan.kind !== 'ready') assert.fail('whole-media-admission-required');
    assert.equal(plan.plan.empty, false);
    assert.equal(plan.plan.hydrate(), true);
    assert.equal(plan.plan.hydrate(), true);
    assert.equal(f.painted.length, 45);
    assert.equal(new Set(f.painted).size, 45);
    assert.ok(f.painted.every((canvas) => canvas.width === 256 && canvas.height === 256));
    const pixels = f.painted.reduce((total, canvas) => total + canvas.width * canvas.height, 0);
    assert.equal(pixels, 2949120);
    assert.ok(pixels <= 4194304);
    assert.equal(f.media.preflightAll().kind, 'retired');
    assert.equal(f.media.prepare(f.roots), null);
  } finally {
    f.media.destroy();
  }
  assert.equal(f.disposals(), 0);
  for (const canvas of f.painted)
    assert.deepEqual([canvas.width, canvas.height, canvas.isConnected], [0, 0, false]);
});

for (const size of [1024, 1025] as const)
  test(`whole preflight ${size === 1024 ? 'aggregate' : 'per-item'} refusal leaves the owner unplanned for one valid selected batch`, () => {
    const f = batch(5, 1024, 1024);
    if (size === 1025) Reflect.set(f.raster, 'width', 1025);
    try {
      assert.equal(f.media.preflightAll().kind, 'deferred');
      assert.equal(f.painted.length, 0);
      if (size === 1025) {
        Reflect.set(f.raster, 'width', 1024);
      }
      const plan = f.media.prepare(f.roots.slice(0, 2));
      assert.ok(plan);
      assert.equal(plan.hydrate(), true);
      assert.equal(f.painted.length, 2);
      assert.equal(f.media.prepare(f.roots.slice(2)), null);
      assert.equal(f.media.preflightAll().kind, 'retired');
    } finally {
      f.media.destroy();
    }
    assert.equal(f.disposals(), 0);
  });

test('whole preflight freezes dimensions before later descriptor mutation and does not reread them during WRITE', () => {
  const f = batch(1, 16, 8);
  let reads = 0;
  Object.defineProperty(f.raster, 'width', {
    configurable: true,
    get() {
      reads++;
      return reads === 1 ? 16 : 1048576;
    },
  });
  try {
    const result = f.media.preflightAll();
    assert.equal(result.kind, 'ready');
    if (result.kind !== 'ready') assert.fail('whole-media-admission-required');
    assert.equal(result.plan.hydrate(), true);
    assert.equal(reads, 1);
    assert.equal(f.painted[0]!.width, 16);
  } finally {
    f.media.destroy();
  }
});

test('dimension callback reentry cannot register, prepare or issue another plan during the temporary whole reservation', () => {
  const f = batch(1, 16, 8);
  Object.defineProperty(f.raster, 'width', {
    configurable: true,
    get() {
      assert.equal(f.media.preflightAll().kind, 'retired');
      assert.equal(f.media.prepare(f.roots), null);
      assert.throws(() => f.media.defer(f.roots[0]!, f.raster), /registration-retired/);
      return 16;
    },
  });
  try {
    const result = f.media.preflightAll();
    assert.equal(result.kind, 'ready');
    if (result.kind !== 'ready') assert.fail('whole-media-admission-required');
    assert.equal(result.plan.hydrate(), true);
    assert.equal(f.painted.length, 1);
  } finally {
    f.media.destroy();
  }
});

test('unexpected dimension failures stay raw and cannot reopen fallback eligibility', () => {
  const f = batch(1, 16, 8),
    cause = new Error('controlled-preflight-dimension-failure');
  Object.defineProperty(f.raster, 'width', {
    get() {
      throw cause;
    },
  });
  try {
    assert.throws(
      () => f.media.preflightAll(),
      (error) => error === cause,
    );
    assert.equal(f.media.prepare(f.roots), null);
    assert.equal(f.media.preflightAll().kind, 'retired');
    assert.equal(f.painted.length, 0);
  } finally {
    f.media.destroy();
  }
});

test('retirement during descriptor capture or before hydration rejects without allocating or granting a fallback plan', () => {
  const first = batch(1, 16, 8);
  Object.defineProperty(first.raster, 'width', {
    get() {
      first.media.destroy();
      return 16;
    },
  });
  assert.equal(first.media.preflightAll().kind, 'retired');
  assert.equal(first.painted.length, 0);
  assert.equal(first.media.prepare(first.roots), null);
  let current = true;
  const second = batch(1, 16, 8, () => current),
    result = second.media.preflightAll();
  assert.equal(result.kind, 'ready');
  if (result.kind !== 'ready') assert.fail('whole-media-admission-required');
  current = false;
  const allocations = second.f.allocations;
  assert.equal(result.plan.hydrate(), false);
  assert.equal(second.f.allocations, allocations);
  assert.equal(second.media.prepare(second.roots), null);
  assert.equal(second.media.preflightAll().kind, 'retired');
  second.media.destroy();
});
