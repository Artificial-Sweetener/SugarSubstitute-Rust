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
/** Literal donor bubble operands and actual finite DOM leases characterize landing without claiming native raster proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import {
  ReorderLandingPaint,
  reorderLandingFragments,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-paint.ts';
import type { ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
const viewport = { left: -1000, right: 1000, top: -1000, bottom: 1000 };
/** Authored fragments are independent viewport CSS-pixel specimens. */
function fragment(
  left: number,
  right: number,
  top: number,
  bottom: number,
  index = 2,
): ReorderRect {
  return { kind: 'chip', index, left, right, top, bottom };
}
test('inflated twelve-pixel bubble gaps join raw twenty-pixel gaps while wrapped rows stay distinct', () => {
  const result = reorderLandingFragments(
    [
      fragment(10, 20, 5, 25),
      fragment(40, 50, 5, 25),
      fragment(10, 70, 25, 45),
      fragment(1, 9, 5, 25, 9),
    ],
    2,
    viewport,
  );
  assert.deepEqual(result, [
    { left: 6, right: 54, top: 3, bottom: 27 },
    { left: 6, right: 74, top: 23, bottom: 47 },
  ]);
  assert.ok(Object.isFrozen(result));
  assert.ok(Object.isFrozen(result[0]));
  assert.deepEqual(
    reorderLandingFragments([fragment(0, 10, 0, 20), fragment(23, 30, 0, 20)], 2, viewport),
    [{ left: -4, right: 34, top: -2, bottom: 22 }],
  );
});
test('twenty-one-pixel raw gaps remain separate and malformed or over-budget fragments cannot publish landing', () => {
  assert.equal(
    reorderLandingFragments([fragment(0, 10, 0, 20), fragment(31, 38, 0, 20)], 2, viewport).length,
    2,
  );
  assert.deepEqual(reorderLandingFragments([fragment(1, Infinity, 0, 20)], 2, viewport), []);
  assert.deepEqual(
    reorderLandingFragments(
      Array.from({ length: 4097 }, () => fragment(0, 1, 0, 1)),
      2,
      viewport,
    ),
    [],
  );
});
test('clipping precedes center-height row admission and preserves tiny protruding rounded corner bubbles', () => {
  assert.deepEqual(
    reorderLandingFragments([fragment(10, 20, 5, 25), fragment(30, 50, 10, 30)], 2, viewport),
    [{ left: 6, right: 54, top: 3, bottom: 32 }],
  );
  assert.deepEqual(
    reorderLandingFragments([fragment(-3, 10, -1, 10), fragment(40, 50, 0, 20)], 2, {
      left: 0,
      right: 45,
      top: 0,
      bottom: 15,
    }),
    [
      { left: 0, right: 14, top: 0, bottom: 12 },
      { left: 36, right: 45, top: 0, bottom: 15 },
    ],
  );
  assert.deepEqual(
    reorderLandingFragments([fragment(0, 100, 0, 20), fragment(0, 1, 0, 1)], 2, viewport),
    [
      { left: -4, right: 5, top: -2, bottom: 3 },
      { left: -4, right: 104, top: -2, bottom: 22 },
    ],
  );
});
test('warm accepted landing keeps its actual union path while opacity and native viewport clipping change', () => {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document),
    bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }];
  landing.show(bubbles, 'rgb(1, 2, 3)', true, viewport);
  const surface = f.document.body!.childNodes.at(-1) as (typeof f.chips)[number],
    svg = surface.childNodes[0] as (typeof f.chips)[number],
    path = svg.childNodes[0] as (typeof f.chips)[number];
  const original = path.getAttribute('d'),
    allocations = f.allocations;
  assert.equal(surface.style.opacity, '0.52');
  assert.equal(path.getAttribute('stroke-width'), '1.25');
  landing.show(bubbles, 'rgb(4, 5, 6)', false, {
    left: 5,
    right: 100,
    top: 10,
    bottom: 80,
  });
  assert.equal(f.document.body!.childNodes.at(-1), surface);
  assert.equal(svg.childNodes[0], path);
  assert.equal(path.getAttribute('d'), original);
  assert.equal(f.allocations, allocations);
  assert.equal(surface.style.opacity, '0.82');
  assert.equal(path.getAttribute('stroke'), 'rgb(4, 5, 6)');
  assert.equal(surface.style.clipPath, 'polygon(5px 10px,100px 10px,100px 80px,5px 80px)');
  landing.clear();
  assert.equal(surface.isConnected, false);
  assert.equal(f.document.body!.childNodes.length, 1);
});
test('missing complete contour retires previous landing rather than keeping stale settled paint', () => {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document);
  landing.show([{ left: 6, right: 34, top: 18, bottom: 42 }], 'black', false, viewport);
  const old = f.document.body!.childNodes.at(-1) as (typeof f.chips)[number];
  landing.show([], 'black', true, viewport);
  assert.equal(old.isConnected, false);
  assert.equal(f.document.body!.childNodes.length, 1);
});
test('detached surface or path is rebuilt before warm geometry can claim visible paint', () => {
  for (const removed of ['surface', 'path'] as const) {
    const f = motionPaintFixture(),
      landing = new ReorderLandingPaint(f.document as unknown as Document),
      bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }];
    landing.show(bubbles, 'black', false, viewport);
    const old = f.document.body!.childNodes.at(-1) as (typeof f.chips)[number],
      svg = old.childNodes[0] as (typeof f.chips)[number],
      path = svg.childNodes[0] as (typeof f.chips)[number];
    (removed === 'surface' ? old : path).remove();
    landing.show(bubbles, 'black', false, viewport);
    const replacement = f.document.body!.childNodes.at(-1) as (typeof f.chips)[number];
    assert.notEqual(replacement, old);
    assert.equal(replacement.isConnected, true);
    assert.equal(old.isConnected, false);
    landing.clear();
  }
});

test('detached contour candidates preserve accepted paint until exact commit, then transfer same-path withdrawal', () => {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document),
    bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }];
  const first = landing.prepare(bubbles, 'black', false, viewport);
  assert.equal(
    first.commit(() => true),
    true,
  );
  const node = f.document.body!.querySelector('.sp-reorder-landing')!,
    path = node.querySelector('[data-reorder-landing-contour]')!,
    allocations = f.allocations;
  const rejected = landing.prepare(
    [{ left: 60, right: 90, top: 20, bottom: 40 }],
    'red',
    false,
    viewport,
  );
  assert.equal(node.isConnected, true);
  assert.equal(
    rejected.commit(() => false),
    false,
  );
  rejected.release();
  assert.equal(first.current(), true);
  const second = landing.prepare(bubbles, 'blue', false, viewport);
  assert.equal(f.allocations, allocations + 3);
  assert.equal(
    second.commit(() => true),
    true,
  );
  first.release();
  assert.equal(first.current(), false);
  assert.equal(second.current(), true);
  assert.equal(f.document.body!.querySelector('.sp-reorder-landing'), node);
  assert.equal(node.querySelector('[data-reorder-landing-contour]'), path);
  assert.equal(path.getAttribute('stroke'), 'blue');
  assert.equal(
    second.refresh(true, () => true),
    true,
  );
  assert.equal(node.style.opacity, '0.52');
  second.release();
  assert.equal(node.isConnected, false);
});

test('unavailable contour commits an explicit empty publication without retaining old pixels', () => {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document),
    first = landing.prepare(
      [{ left: 6, right: 34, top: 18, bottom: 42 }],
      'black',
      false,
      viewport,
    );
  assert.equal(
    first.commit(() => true),
    true,
  );
  const old = f.document.body!.querySelector('.sp-reorder-landing')!;
  const unavailable = landing.prepare(
    Array.from({ length: 257 }, () => ({
      left: 0,
      right: 10,
      top: 0,
      bottom: 10,
    })),
    'black',
    false,
    viewport,
  );
  assert.equal(old.isConnected, true);
  assert.equal(
    unavailable.commit(() => true),
    true,
  );
  first.release();
  assert.equal(unavailable.current(), true);
  assert.equal(old.isConnected, false);
  assert.equal(f.document.body!.querySelector('.sp-reorder-landing'), null);
  unavailable.release();
  assert.equal(unavailable.current(), false);
});

for (const raw of [null, undefined])
  test(`contour native write preserves raw ${String(raw)} and attempts captured retirement`, () => {
    const f = motionPaintFixture(),
      landing = new ReorderLandingPaint(f.document as unknown as Document),
      bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }];
    landing.show(bubbles, 'black', false, viewport);
    const node = f.document.body!.querySelector('.sp-reorder-landing')!,
      path = node.querySelector('[data-reorder-landing-contour]')!,
      second = landing.prepare(bubbles, 'blue', false, viewport);
    path.setAttribute = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw raw;
    };
    assert.throws(
      () => second.commit(() => true),
      (cause: unknown) => cause === raw,
    );
    assert.equal(second.current(), false);
    assert.equal(node.isConnected, false);
  });

test('reentrant contour commit cannot resume writes into or withdraw the newer surface', () => {
  const f = motionPaintFixture(),
    landing = new ReorderLandingPaint(f.document as unknown as Document),
    bubbles = [{ left: 6, right: 34, top: 18, bottom: 42 }];
  landing.show(bubbles, 'black', false, viewport);
  const old = f.document.body!.querySelector('.sp-reorder-landing')!,
    candidate = landing.prepare(bubbles, 'blue', false, viewport);
  let opacity = old.style.opacity;
  Object.defineProperty(old.style, 'opacity', {
    configurable: true,
    get: () => opacity,
    set(value: string) {
      landing.show(bubbles, 'green', false, viewport);
      opacity = value;
    },
  });
  assert.equal(
    candidate.commit(() => true),
    false,
  );
  const newer = f.document.body!.querySelector('.sp-reorder-landing')!;
  assert.notEqual(newer, old);
  assert.equal(old.isConnected, false);
  assert.equal(
    newer.querySelector('[data-reorder-landing-contour]')!.getAttribute('stroke'),
    'green',
  );
  candidate.release();
  assert.equal(newer.isConnected, true);
  landing.clear();
});
