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
import { resolveReorderPlacement } from '../../../../../../frontend/sugarprompt/editor/src/reorder-placement-topology.ts';
import { literalPlacementTopology } from './reorder-placement-fixture.ts';
/** Literal independently measured rectangles prove finite hidden-layout adaptation without claiming native geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { type ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import { reorderKeyboardTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-keyboard-geometry.ts';
import { reorderState, captureReorder } from '../transaction/reorder-fixture.ts';

test('pointer targets real chip halves with exact Core insertion ordinals', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 0)!;
  const rects: readonly ReorderRect[] = [
    { kind: 'chip', index: 1, left: 10, right: 30, top: 10, bottom: 30 },
    { kind: 'chip', index: 2, left: 40, right: 60, top: 10, bottom: 30 },
  ];
  assert.deepEqual(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: rects,
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 12, y: 20 },
      null,
      null,
    )?.target ?? null,
    {
      kind: 'row',
      row: 0,
      insertion: 0,
    },
  );
  assert.deepEqual(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: rects,
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 55, y: 20 },
      null,
      null,
    )?.target ?? null,
    {
      kind: 'row',
      row: 0,
      insertion: 2,
    },
  );
});

test('vertical line intent precedes horizontal proximity to a wider upper line', () => {
  const capture = captureReorder(
      reorderState('soft light, subject\nA place to write, select and revise.').state,
    ),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 3)!,
    rects: readonly ReorderRect[] = [
      { kind: 'chip', index: 0, left: 10, right: 95, top: 10, bottom: 30 },
      { kind: 'chip', index: 1, left: 100, right: 200, top: 10, bottom: 30 },
      { kind: 'chip', index: 2, left: 10, right: 120, top: 35, bottom: 55 },
    ];
  for (const y of [45, 65])
    assert.deepEqual(
      resolveReorderPlacement(
        literalPlacementTopology(valueReorderLayout(hidden), {
          rects: rects,
          viewport: { left: 0, right: 200, top: 0, bottom: 200 },
        }),
        { x: 180, y: y },
        null,
        null,
      )?.target ?? null,
      { kind: 'row', row: 1, insertion: 1 },
    );
});

test('wrapped physical lines select their own measured insertion before horizontal distance', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 2)!,
    rects: readonly ReorderRect[] = [
      { kind: 'chip', index: 0, left: 10, right: 90, top: 10, bottom: 30 },
      { kind: 'chip', index: 1, left: 10, right: 30, top: 35, bottom: 55 },
    ];
  assert.deepEqual(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: rects,
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 70, y: 45 },
      null,
      null,
    )?.target ?? null,
    { kind: 'row', row: 0, insertion: 2 },
  );
});

test('blank destinations retain their literal ordinal and horizontal neighbors require their own measured identity', () => {
  const capture = captureReorder(reorderState('a\n\nb, c').state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 2)!,
    gap = valueReorderLayout(hidden).gaps.find((candidate) => candidate.blankOffsets.length)!;
  const rects: readonly ReorderRect[] = [
    {
      kind: 'gap',
      index: gap.index,
      blank: 0,
      left: 0,
      right: 10,
      top: 20,
      bottom: 30,
    },
    { kind: 'chip', index: 1, left: 0, right: 10, top: 40, bottom: 50 },
  ];
  assert.deepEqual(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: rects,
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 5, y: 25 },
      null,
      null,
    )?.target ?? null,
    {
      kind: 'gap',
      gap: gap.index,
      blank: 0,
    },
  );
  assert.deepEqual(
    reorderKeyboardTarget(
      capture.prepared.initial,
      hidden,
      2,
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects,
        viewport: { left: 0, right: 100, top: 0, bottom: 120 },
      }),
      'ArrowLeft',
      null,
      { x: 5, y: 50 },
    )?.target ?? null,
    { kind: 'row', row: 1, insertion: 0 },
  );
  assert.equal(
    reorderKeyboardTarget(
      capture.prepared.initial,
      hidden,
      2,
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects,
        viewport: { left: 0, right: 100, top: 0, bottom: 120 },
      }),
      'ArrowRight',
      null,
      { x: 5, y: 50 },
    )?.target ?? null,
    null,
  );
  assert.deepEqual(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: [
          {
            kind: 'partition',
            index: 2,
            left: 0,
            right: 10,
            top: 10,
            bottom: 20,
          },
        ],
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 5, y: 15 },
      null,
      null,
    )?.target ?? null,
    { kind: 'partition', partition: 2 },
  );
});
test('vertical intent chooses the nearest measured wrapped row at retained x and unavailable geometry refuses', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    hidden = hideReorderChip(capture.prepared, capture.prepared.initial, 0)!;
  const rects: readonly ReorderRect[] = [
    { kind: 'chip', index: 1, left: 10, right: 30, top: 10, bottom: 30 },
    { kind: 'chip', index: 2, left: 40, right: 60, top: 40, bottom: 60 },
  ];
  assert.deepEqual(
    reorderKeyboardTarget(
      capture.prepared.initial,
      hidden,
      0,
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects,
        viewport: { left: 0, right: 60, top: 0, bottom: 120 },
      }),
      'ArrowDown',
      50,
      { x: 50, y: 20 },
    )?.target ?? null,
    { kind: 'row', row: 0, insertion: 2 },
  );
  assert.equal(
    resolveReorderPlacement(
      literalPlacementTopology(valueReorderLayout(hidden), {
        rects: [],
        viewport: { left: 0, right: 200, top: 0, bottom: 200 },
      }),
      { x: 10, y: 10 },
      null,
      null,
    )?.target ?? null,
    null,
  );
});
