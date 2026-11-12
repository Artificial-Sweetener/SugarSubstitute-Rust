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
import { literalPlacementTopology } from './reorder-placement-fixture.ts';
/** Literal lane geometry proves donor semantic occurrence rules independently of browser timing. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { reorderKeyboardTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-keyboard-geometry.ts';
import {
  reorderHeldSlot,
  reorderHeldMarker,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
/** Independent CSS-pixel fragments include repeated wrapped occurrences of one actual chip. */
function operands() {
  const capture = captureReorder(reorderState('a, b, c').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 0)!;
  const rects: readonly ReorderRect[] = [
    { kind: 'chip', index: 1, left: 10, right: 30, top: 10, bottom: 30 },
    { kind: 'chip', index: 1, left: 110, right: 130, top: 40, bottom: 60 },
    { kind: 'chip', index: 2, left: 10, right: 30, top: 70, bottom: 90 },
  ];
  return {
    order,
    hidden,
    geometry: { rects, viewport: { left: 0, right: 200, top: 0, bottom: 120 } },
  };
}
test('initial live center selects the nearest current wrapped semantic occurrence', () => {
  const { order, hidden, geometry } = operands();
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowDown',
      null,
      {
        x: 120,
        y: 50,
      },
    ),
    { target: { kind: 'row', row: 0, insertion: 1 }, preferredX: 10 },
  );
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowDown',
      null,
      {
        x: 20,
        y: 20,
      },
    ),
    { target: { kind: 'row', row: 0, insertion: 0 }, preferredX: 60 },
  );
});
test('current preferred x supersedes initial center and stores the destination hit-region center', () => {
  const { order, hidden, geometry } = operands();
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowDown',
      100,
      {
        x: 20,
        y: 20,
      },
    ),
    { target: { kind: 'row', row: 0, insertion: 2 }, preferredX: 110 },
  );
});
test('horizontal wrapped seams skip repeated targets in physical reading order', () => {
  const { order, hidden, geometry } = operands();
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowRight',
      null,
      {
        x: 20,
        y: 20,
      },
    ),
    { target: { kind: 'row', row: 0, insertion: 1 }, preferredX: 110 },
  );
  assert.equal(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowLeft',
      null,
      {
        x: 20,
        y: 20,
      },
    ),
    null,
  );
});
test('vertical boundary clamps to the current physical lane edge rather than dropping the valid key', () => {
  const { order, hidden, geometry } = operands();
  assert.equal(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowUp',
      10,
      {
        x: 20,
        y: 20,
      },
    ),
    null,
  );
  const bottom = {
    ...geometry,
    rects: [
      {
        kind: 'chip' as const,
        index: 1,
        left: 10,
        right: 30,
        top: 10,
        bottom: 30,
      },
    ],
  };
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), bottom),
      'ArrowDown',
      null,
    ),
    { target: { kind: 'row', row: 0, insertion: 1 }, preferredX: 110 },
  );
});
test('keyboard geometry is not a clipped marker center and offscreen insertion remains measurable', () => {
  const { order, hidden, geometry } = operands(),
    viewport = { left: 0, right: 200, top: 100, bottom: 120 };
  assert.deepEqual(reorderHeldSlot(order, hidden, 0, geometry.rects), {
    left: 10,
    right: 12,
    top: 10,
    bottom: 30,
  });
  assert.equal(reorderHeldMarker(order, hidden, 0, geometry.rects, viewport), null);
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), {
        ...geometry,
        viewport,
      }),
      'ArrowRight',
      null,
      { x: 20, y: 20 },
    ),
    { target: { kind: 'row', row: 0, insertion: 1 }, preferredX: 110 },
  );
});
test('empty or malformed viewport geometry refuses without inferred widths', () => {
  const { order, hidden, geometry } = operands();
  assert.equal(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), {
        ...geometry,
        rects: [],
      }),
      'ArrowDown',
      null,
    ),
    null,
  );
  assert.equal(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), {
        ...geometry,
        viewport: { left: 0, right: NaN, top: 0, bottom: 120 },
      }),
      'ArrowDown',
      null,
    ),
    null,
  );
});

test('final singleton resolves the greatest genuine trailing blank occurrence before vertical movement', () => {
  const capture = captureReorder(reorderState('a\nb').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 1)!;
  const geometry = {
    viewport: { left: 0, right: 200, top: 0, bottom: 120 },
    rects: [
      {
        kind: 'chip' as const,
        index: 0,
        left: 10,
        right: 30,
        top: 10,
        bottom: 30,
      },
      {
        kind: 'gap' as const,
        index: 0,
        blank: 0,
        left: 0,
        right: 2,
        top: 10,
        bottom: 30,
      },
      {
        kind: 'gap' as const,
        index: 0,
        blank: 1,
        left: 0,
        right: 2,
        top: 40,
        bottom: 60,
      },
    ],
  };
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      1,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowUp',
      null,
    ),
    { target: { kind: 'row', row: 0, insertion: 0 }, preferredX: 10 },
  );
});

test('empty regional partitions are current independent lanes and remain genuine Core destinations', () => {
  const capture = captureReorder(reorderState('a\n[SEP|Face]\nb').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 0)!;
  const geometry = {
    viewport: { left: 0, right: 200, top: 0, bottom: 120 },
    rects: [
      {
        kind: 'partition' as const,
        index: 0,
        left: 0,
        right: 2,
        top: 10,
        bottom: 30,
      },
      {
        kind: 'chip' as const,
        index: 1,
        left: 10,
        right: 30,
        top: 10,
        bottom: 30,
      },
    ],
  };
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowUp',
      null,
      {
        x: 20,
        y: 20,
      },
    ),
    null,
  );
});

test('distinct Core regional rows retain separate lanes even with equal physical tops', () => {
  const capture = captureReorder(reorderState('a, b\n[SEP|Face]\nc').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 0)!;
  const geometry = {
    viewport: { left: 0, right: 200, top: 0, bottom: 120 },
    rects: [
      {
        kind: 'chip' as const,
        index: 1,
        left: 10,
        right: 30,
        top: 10,
        bottom: 30,
      },
      {
        kind: 'chip' as const,
        index: 2,
        left: 40,
        right: 60,
        top: 10,
        bottom: 30,
      },
    ],
  };
  assert.deepEqual(
    reorderKeyboardTarget(
      order,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), geometry),
      'ArrowDown',
      null,
      {
        x: 20,
        y: 20,
      },
    ),
    { target: { kind: 'row', row: 1, insertion: 0 }, preferredX: 25 },
  );
});
