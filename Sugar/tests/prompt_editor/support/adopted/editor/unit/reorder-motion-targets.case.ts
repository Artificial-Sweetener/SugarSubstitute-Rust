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
/** Pure actual CSS-pixel endpoint plans distinguish observation breadth from finite animated output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
/** Independent multirow native rectangles include repeated fragments in physical RTL order. */
function rect(id: number, left = (id % 13) * 30, top = Math.floor(id / 13) * 30): ReorderRect {
  return Object.freeze({
    kind: 'chip',
    index: id,
    left,
    right: left + 20,
    top,
    bottom: top + 20,
  });
}

test('130 observed groups prepare only the two actual translated peers with held polarity and frozen borrowed geometry', () => {
  const geometry = Object.freeze(Array.from({ length: 130 }, (_, id) => rect(id))),
    origin = geometry.map((value) =>
      value.index === 2 || value.index === 90
        ? { ...value, left: value.left + 10, right: value.right + 10 }
        : value,
    ),
    targets = reorderMotionTargets(origin, geometry, 90);
  assert.ok(targets);
  assert.deepEqual(
    targets.map((target) => [target.id, target.kind]),
    [
      [2, 'peer'],
      [90, 'held'],
    ],
  );
  assert.deepEqual(targets[0], {
    id: 2,
    kind: 'peer',
    from: { left: 70, top: 0 },
    to: { left: 60, top: 0 },
  });
  assert.equal(Object.isFrozen(targets), true);
  assert.equal(Object.isFrozen(targets[0]), true);
  assert.equal(Object.isFrozen(targets[0]!.from), true);
  assert.equal(Object.isFrozen(targets[0]!.to), true);
  assert.equal(geometry.length, 130);
  assert.deepEqual(reorderMotionTargets(geometry, geometry, null), []);
});

test('64 actually changed groups fit; 65 refuse the entire plan rather than truncate by order', () => {
  for (const count of [64, 65]) {
    const geometry = Array.from({ length: 130 }, (_, id) => rect(id)),
      origin = geometry.map((value) =>
        value.index < count ? { ...value, left: value.left + 1 } : value,
      ),
      actual = reorderMotionTargets(origin, geometry, null);
    if (count === 64) assert.equal(actual?.length, 64);
    else assert.equal(actual, null);
  }
});

test('wrapped RTL groups use every actual fragment for their anchor while missing origins and shape-only changes remain native', () => {
  const geometry = [
      rect(7, 90, 20),
      rect(7, 10, 40),
      rect(8, 50, 60),
      rect(9, 70, 20),
      rect(10, 20, 20),
    ],
    origin = [rect(7, 80, 10), rect(7, 20, 30), rect(8, 50, 60), rect(8, 80, 80), rect(10, 20, 20)],
    targets = reorderMotionTargets(origin, geometry, null);
  assert.deepEqual(targets, [
    {
      id: 7,
      kind: 'peer',
      from: { left: 20, top: 10 },
      to: { left: 10, top: 20 },
    },
  ]);
  assert.deepEqual(reorderMotionTargets([], geometry, null), []);
});

for (const invalid of [NaN, Infinity, -Infinity])
  test(`unexpected nonfinite endpoint remains observable for ${String(invalid)}`, () => {
    assert.throws(() => reorderMotionTargets([{ ...rect(1), left: invalid }], [rect(1)], null), {
      name: 'TypeError',
      message: 'reorder-motion-geometry',
    });
    assert.throws(() => reorderMotionTargets([rect(1)], [{ ...rect(1), bottom: invalid }], null), {
      name: 'TypeError',
      message: 'reorder-motion-geometry',
    });
  });

test('invalid semantic IDs remain unexpected numeric-boundary failures', () => {
  for (const id of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1])
    assert.throws(() => reorderMotionTargets([], [rect(id)], null), {
      name: 'TypeError',
      message: 'reorder-motion-geometry',
    });
});
