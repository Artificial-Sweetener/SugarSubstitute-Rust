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
/** Literal CSS-pixel bubbles prove rounded union topology independently of native source/layout owners. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareReorderLandingContour,
  type ReorderLandingBubble,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-landing-geometry.ts';
/** Independent specimens are already inflated/clipped paint bounds. */
function bubble(left: number, right: number, top: number, bottom: number): ReorderLandingBubble {
  return { left, right, top, bottom };
}
test('one ordinary bubble has its exact radius and closed rounded exterior', () => {
  assert.deepEqual(prepareReorderLandingContour([bubble(6, 34, 18, 42)]), {
    status: 'ready',
    left: 5,
    top: 17,
    width: 30,
    height: 26,
    path: 'M15 18L25 18A9 9 0 0 1 34 27L34 33A9 9 0 0 1 25 42L15 42A9 9 0 0 1 6 33L6 27A9 9 0 0 1 15 18Z',
  });
});
test('coincident copies retain one exterior while disconnected bubbles retain separate closed components', () => {
  const single = prepareReorderLandingContour([bubble(6, 34, 18, 42)]);
  assert.deepEqual(
    prepareReorderLandingContour([bubble(6, 34, 18, 42), bubble(6, 34, 18, 42)]),
    single,
  );
  const disconnected = prepareReorderLandingContour([
    bubble(6, 34, 18, 42),
    bubble(66, 94, 18, 42),
  ]);
  assert.equal(disconnected.status, 'ready');
  if (disconnected.status === 'ready') {
    assert.equal(disconnected.path.split('M').length - 1, 2);
    assert.equal(disconnected.path.split('Z').length - 1, 2);
    assert.equal(disconnected.width, 90);
  }
});
test('overlapping wrapped bubbles have one exterior without the shared horizontal seam', () => {
  const contour = prepareReorderLandingContour([bubble(6, 34, 18, 42), bubble(6, 54, 38, 62)]);
  assert.equal(contour.status, 'ready');
  if (contour.status === 'ready') {
    assert.equal(contour.path.split('M').length - 1, 1);
    assert.equal(contour.path.split('Z').length - 1, 1);
    assert.equal(contour.path.includes('L15 42'), false);
    assert.deepEqual([contour.left, contour.top, contour.width, contour.height], [5, 17, 50, 46]);
  }
});
test('fractional narrow bubbles use their actual constrained radius without a product minimum width', () => {
  const contour = prepareReorderLandingContour([bubble(-3.75, 5.75, -0.5, 5.5)]);
  assert.equal(contour.status, 'ready');
  if (contour.status === 'ready') {
    assert.deepEqual(
      [contour.left, contour.top, contour.width, contour.height],
      [-4.75, -1.5, 11.5, 8],
    );
    assert.ok(contour.path.includes('A3 3'));
  }
});
test('invalid and excessive complete footprints settle explicit finite paint unavailability', () => {
  assert.deepEqual(prepareReorderLandingContour([]), {
    status: 'unavailable',
    reason: 'fragment-budget',
  });
  assert.deepEqual(prepareReorderLandingContour([bubble(0, Infinity, 0, 20)]), {
    status: 'unavailable',
    reason: 'invalid-geometry',
  });
  assert.deepEqual(
    prepareReorderLandingContour(Array.from({ length: 257 }, () => bubble(0, 20, 0, 20))),
    { status: 'unavailable', reason: 'fragment-budget' },
  );
  assert.deepEqual(
    prepareReorderLandingContour(Array.from({ length: 256 }, () => bubble(0, 20, 0, 20))),
    { status: 'unavailable', reason: 'work-budget' },
  );
});
test('opposite-facing coincident shared edges disappear from both rounded bubbles', () => {
  const contour = prepareReorderLandingContour([bubble(-4, 24, -2, 22), bubble(24, 52, 2, 26)]);
  assert.equal(contour.status, 'ready');
  if (contour.status === 'ready') {
    assert.equal(contour.path.split('M').length - 1, 1);
    assert.equal(contour.path.split('Z').length - 1, 1);
    assert.equal(
      contour.path,
      'M5 -2L15 -2A9 9 0 0 1 24 7L24 11A9 9 0 0 1 33 2L43 2A9 9 0 0 1 52 11L52 17A9 9 0 0 1 43 26L33 26A9 9 0 0 1 24 17L24 13A9 9 0 0 1 15 22L5 22A9 9 0 0 1 -4 13L-4 7A9 9 0 0 1 5 -2Z',
    );
    assert.deepEqual([contour.left, contour.top, contour.width, contour.height], [-5, -3, 58, 30]);
  }
});
test('finite coordinates with unrepresentable extents cannot emit infinite commands', () => {
  for (const rect of [bubble(-1e308, 1e308, 0, 20), bubble(1e308, 1.1e308, 0, 20)])
    assert.deepEqual(prepareReorderLandingContour([rect]), {
      status: 'unavailable',
      reason: 'invalid-geometry',
    });
});
