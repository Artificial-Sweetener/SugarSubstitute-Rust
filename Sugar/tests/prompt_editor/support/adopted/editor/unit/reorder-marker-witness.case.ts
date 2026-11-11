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
/** Pure Core marker witness selection preserves native fragment identity and exact fallback affinity independently of pointer scheduling. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  dropReorderChip,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  reorderHeldMarker,
  reorderMarkerWitnesses,
  selectReorderMarkerTargets,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
const viewport = { left: 0, right: 100, top: 0, bottom: 100 };
test('one Core witness owner preserves held-first, next-first, previous-last, exact blank and partition selection', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    initial = capture.prepared.initial;
  const hidden = hideReorderChip(capture.prepared, initial, 0)!;
  const next = reorderMarkerWitnesses(initial, hidden, 0)!;
  assert.deepEqual(next, {
    held: 0,
    fallback: { kind: 'chip', index: 1 },
    trailing: false,
  });
  const moved = dropReorderChip(capture.prepared, hidden, 0, {
    kind: 'row',
    row: 0,
    insertion: 2,
  })!;
  const previous = reorderMarkerWitnesses(moved, hideReorderChip(capture.prepared, moved, 0)!, 0)!;
  assert.deepEqual(previous, {
    held: 0,
    fallback: { kind: 'chip', index: 2 },
    trailing: true,
  });
  const blankCapture = captureReorder(reorderState('a\nb\nc').state),
    blankOrder = blankCapture.prepared.initial;
  const blank = reorderMarkerWitnesses(
    blankOrder,
    hideReorderChip(blankCapture.prepared, blankOrder, 1)!,
    1,
  )!;
  assert.deepEqual(blank, {
    held: 1,
    fallback: { kind: 'gap', index: 0, blank: 0 },
    trailing: false,
  });
  const regional = captureReorder(reorderState('a\n[SEP]\nb').state),
    regionalOrder = regional.prepared.initial;
  const partition = reorderMarkerWitnesses(
    regionalOrder,
    hideReorderChip(regional.prepared, regionalOrder, 0)!,
    0,
  )!;
  assert.deepEqual(partition, {
    held: 0,
    fallback: { kind: 'partition', index: 0 },
    trailing: false,
  });
  const paint: readonly ReorderRect[] = [
    { kind: 'chip', index: 0, left: 10, right: 20, top: 10, bottom: 30 },
    { kind: 'chip', index: 1, left: 30, right: 40, top: 10, bottom: 30 },
    { kind: 'chip', index: 1, left: 0, right: 20, top: 30, bottom: 50 },
    { kind: 'chip', index: 2, left: 30, right: 40, top: 30, bottom: 50 },
    { kind: 'chip', index: 2, left: 0, right: 15, top: 50, bottom: 70 },
    { kind: 'gap', index: 0, blank: 1, left: 1, right: 2, top: 20, bottom: 40 },
    { kind: 'gap', index: 0, blank: 0, left: 5, right: 6, top: 20, bottom: 40 },
    { kind: 'partition', index: 0, left: 7, right: 8, top: 20, bottom: 40 },
  ];
  assert.equal(selectReorderMarkerTargets(paint, blank).includes(paint[5]!), false);
  assert.equal(selectReorderMarkerTargets(paint, blank).includes(paint[6]!), true);
  assert.equal(selectReorderMarkerTargets(paint, partition).includes(paint[7]!), true);
  assert.deepEqual(
    reorderHeldMarker(initial, hidden, 0, selectReorderMarkerTargets(paint, next), viewport),
    { left: 10, right: 12, top: 10, bottom: 30 },
  );
  const absent = paint.filter((rect) => rect.kind !== 'chip' || rect.index !== 0);
  assert.deepEqual(
    reorderHeldMarker(initial, hidden, 0, selectReorderMarkerTargets(absent, next), viewport),
    { left: 30, right: 32, top: 10, bottom: 30 },
  );
  assert.deepEqual(
    reorderHeldMarker(
      moved,
      hideReorderChip(capture.prepared, moved, 0)!,
      0,
      selectReorderMarkerTargets(absent, previous),
      viewport,
    ),
    { left: 15, right: 17, top: 50, bottom: 70 },
  );
  assert.deepEqual(
    reorderHeldMarker(
      regionalOrder,
      hideReorderChip(regional.prepared, regionalOrder, 0)!,
      0,
      selectReorderMarkerTargets(absent, partition),
      viewport,
    ),
    { left: 7, right: 9, top: 20, bottom: 40 },
  );
  const missingBlankHeld = paint.filter((rect) => rect.kind !== 'chip' || rect.index !== 1);
  assert.deepEqual(
    reorderHeldMarker(
      blankOrder,
      hideReorderChip(blankCapture.prepared, blankOrder, 1)!,
      1,
      selectReorderMarkerTargets(missingBlankHeld, blank),
      viewport,
    ),
    { left: 5, right: 7, top: 20, bottom: 40 },
  );
  assert.equal(reorderMarkerWitnesses(initial, initial, 0), null);
});
