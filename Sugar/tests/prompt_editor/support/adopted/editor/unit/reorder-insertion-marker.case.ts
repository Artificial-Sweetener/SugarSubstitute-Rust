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
/** Literal measured fragments and real native sessions prove insertion feedback without native rendering authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  hideReorderChip,
  dropReorderChip,
  valueReorderLayout,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  reorderHeldMarker,
  type ReorderRect,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
import {
  reorderPreview,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { dragPointer, pointer, key } from './reorder-session-fixture.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth } from '@codemirror/commands';
import { paintFixture, drain, geometryUpdate } from './reorder-marker-fixture.ts';

const viewport = { left: 0, right: 100, top: 0, bottom: 100 };
const rects: readonly ReorderRect[] = [
  { kind: 'chip', index: 1, left: 10, right: 30, top: 10, bottom: 30 },
  { kind: 'chip', index: 2, left: 40, right: 60, top: 10, bottom: 30 },
];

test('before and after marker are derived from actual full Core order and hidden measured fragments', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    original = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, original, 0)!;
  assert.deepEqual(reorderHeldMarker(original, hidden, 0, rects, viewport), {
    left: 10,
    right: 12,
    top: 10,
    bottom: 30,
  });
  const moved = dropReorderChip(capture.prepared, hidden, 0, {
      kind: 'row',
      row: 0,
      insertion: 2,
    })!,
    nextHidden = hideReorderChip(capture.prepared, moved, 0)!;
  assert.deepEqual(reorderHeldMarker(moved, nextHidden, 0, rects, viewport), {
    left: 60,
    right: 62,
    top: 10,
    bottom: 30,
  });
  assert.equal(reorderHeldMarker(original, original, 0, rects, viewport), null);
  assert.equal(reorderHeldMarker(original, hidden, 0, [], viewport), null);
});

test('wrapping scroll and transformed physical fragments use current measured boundaries and clip the complete marker', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 0)!;
  const wrapped = [
    {
      kind: 'chip' as const,
      index: 1,
      left: 35,
      right: 80,
      top: -5,
      bottom: 15,
    },
    {
      kind: 'chip' as const,
      index: 1,
      left: 0,
      right: 20,
      top: 16,
      bottom: 36,
    },
  ];
  assert.deepEqual(reorderHeldMarker(order, hidden, 0, wrapped, viewport), {
    left: 35,
    right: 37,
    top: 0,
    bottom: 15,
  });
  const shifted = wrapped.map((r) => ({
    ...r,
    left: r.left + 100,
    right: r.right + 100,
  }));
  assert.equal(reorderHeldMarker(order, hidden, 0, shifted, viewport), null);
  assert.equal(reorderHeldMarker(order, hidden, 0, rects, { ...viewport, right: NaN }), null);
});

test('a sole held row maps to actual Core blank ordinal and actual empty regional target', () => {
  const capture = captureReorder(reorderState('a\nb\nc').state),
    order = capture.prepared.initial,
    hidden = hideReorderChip(capture.prepared, order, 1)!,
    gap = valueReorderLayout(hidden).gaps[0]!;
  assert.deepEqual(
    reorderHeldMarker(
      order,
      hidden,
      1,
      [
        {
          kind: 'gap',
          index: gap.index,
          blank: 0,
          left: 5,
          right: 17,
          top: 30,
          bottom: 50,
        },
      ],
      viewport,
    ),
    { left: 5, right: 7, top: 30, bottom: 50 },
  );
  assert.equal(
    reorderHeldMarker(
      order,
      hidden,
      1,
      [
        {
          kind: 'gap',
          index: gap.index,
          blank: 1,
          left: 5,
          right: 17,
          top: 30,
          bottom: 50,
        },
      ],
      viewport,
    ),
    null,
  );
  const regional = captureReorder(reorderState('a\n[SEP]\nb').state),
    r = regional.prepared.initial,
    h = hideReorderChip(regional.prepared, r, 0)!;
  assert.deepEqual(
    reorderHeldMarker(
      r,
      h,
      0,
      [
        {
          kind: 'partition',
          index: 0,
          left: 7,
          right: 19,
          top: 20,
          bottom: 40,
        },
      ],
      viewport,
    ),
    { left: 7, right: 9, top: 20, bottom: 40 },
  );
});

test('actual pointer publication remeasures active marker and release retires both paint nodes with one source event', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    const marker = nodes.find((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)!;
    assert.ok(marker);
    assert.equal(marker.style.left, '10px');
    f.owner.pointermove(pointer(1, 20, 55));
    assert.equal(marker.removed, false);
    assert.equal(marker.style.opacity, '0.52');
    drain(f);
    assert.equal(marker.removed, true);
    assert.equal(
      nodes.findLast((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)?.style
        .left,
      '70px',
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    f.owner.pointerup(pointer(1, 20, 55));
    drain(f);
    assert.equal(readExactSource(f.state), 'b, c, a');
    assert.equal(undoDepth(f.state), 1);
    assert.equal(
      nodes.every((node) => node.removed),
      true,
    );
    assert.equal(f.state.field(reorderPreview).value, null);
  } finally {
    f.destroy();
  }
});

test('cancel disposal and source retirement invalidate pending marker reads without reviving resources', () => {
  for (const mode of ['cancel', 'dispose', 'source'] as const) {
    const { f, nodes } = paintFixture();
    try {
      f.owner.pointerdown(pointer());
      drain(f);
      dragPointer(f.owner, pointer());
      drain(f);
      f.owner.pointermove(pointer(1, 20, 55));
      f.measure();
      f.task();
      if (mode === 'cancel') f.owner.keydown(key('Escape'));
      else if (mode === 'dispose') f.owner.destroy();
      else f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      drain(f);
      assert.equal(
        nodes.every((node) => node.removed),
        true,
      );
      assert.equal(f.captures.size, 0);
      assert.equal(f.frames.size, 0);
      assert.equal(readExactSource(f.state), mode === 'source' ? 'xa, b, c' : 'a, b, c');
    } finally {
      f.destroy();
    }
  }
});

test('new pointer intent retains dim accepted geometry until a genuinely newer publication', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    // A real append and authored-vacancy return retain canonical publication; its genuine paint lease owns accepted unavailability.
    f.owner.pointermove(pointer(1, 20, 55));
    drain(f);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [1, 2, 0]);
    f.owner.pointermove(pointer(1, 20, 5));
    drain(f);
    assert.equal(f.state.field(reorderPreview).native, false);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [0, 1, 2]);
    f.owner.pointermove(pointer(1, 20, 5));
    f.measure();
    f.task();
    assert.equal(f.jobs.length, 1);
    const pending = f.jobs.shift()!;
    const measured = pending.read(f.view);
    assert.ok(measured && typeof measured === 'object' && 'rect' in measured);
    assert.deepEqual(measured.rect, {
      left: 10,
      right: 12,
      top: 10,
      bottom: 30,
    });
    f.owner.pointermove(pointer(1, 20, 55));
    pending.write!(measured, f.view);
    const retained = nodes.findLast(
      (n) => n.className === 'sp-reorder-insertion-marker' && !n.removed,
    );
    assert.ok(retained);
    assert.equal(retained.style.left, '10px');
    assert.equal(retained.style.opacity, '0.52');
    drain(f);
    assert.equal(retained.removed, true);
    assert.equal(
      nodes.findLast((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)?.style
        .left,
      '70px',
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('marker append failure preserves its cause and retires proxy capture and frame', () => {
  const { f, nodes } = paintFixture(),
    cause = new Error('marker-append');
  const append = f.view.dom.ownerDocument.body.append;
  let count = 0;
  f.view.dom.ownerDocument.body.append = (...items) => {
    if (++count === 2) throw cause;
    append(...items);
  };
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    assert.throws(
      () => drain(f),
      (error) => error === cause,
    );
    drain(f);
    assert.equal(
      nodes.every((node) => node.removed),
      true,
    );
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('fresh marker viewport read failure retires its finite ownership with the original cause', () => {
  const { f, nodes } = paintFixture(),
    cause = new Error('marker-viewport'),
    read = f.view.scrollDOM.getBoundingClientRect;
  let reads = 0;
  f.view.scrollDOM.getBoundingClientRect = () => {
    if (++reads === 2) throw cause;
    return read();
  };
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    assert.throws(
      () => drain(f),
      (error) => error === cause,
    );
    drain(f);
    assert.equal(
      nodes.every((node) => node.removed),
      true,
    );
    assert.equal(f.captures.size, 0);
    assert.equal(f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
/** Current native update flags must refresh a stationary held marker without reapplying its semantic destination. */
test('stationary held pointer geometry and viewport updates remeasure current publication', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    const before = f.state.field(reorderPreview).value;
    const old = nodes.find((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)!;
    f.view.scrollDOM.getBoundingClientRect = () => ({
      left: 0,
      right: 100,
      top: 20,
      bottom: 100,
      width: 100,
      height: 80,
      x: 0,
      y: 20,
      toJSON() {
        return {};
      },
    });
    f.owner.update(geometryUpdate(f, true, false));
    assert.equal(old.removed, true);
    drain(f);
    assert.equal(
      nodes.findLast((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)?.style.top,
      '20px',
    );
    assert.equal(f.state.field(reorderPreview).value, before);
    f.view.scrollDOM.getBoundingClientRect = () => ({
      left: 0,
      right: 100,
      top: 25,
      bottom: 100,
      width: 100,
      height: 75,
      x: 0,
      y: 25,
      toJSON() {
        return {};
      },
    });
    f.owner.update(geometryUpdate(f, false, true));
    drain(f);
    assert.equal(
      nodes.findLast((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)?.style.top,
      '25px',
    );
    assert.equal(f.state.field(reorderPreview).value, before);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('current geometry refresh ignores its own preview publication and stale retired input', () => {
  const { f, nodes } = paintFixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    const publication = f.state.field(reorderPreview).value!;
    const marker = nodes.find((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed)!;
    const tx = f.state.update({
      effects: installReorderPreview.of(publication),
    });
    f.view.dispatch(tx);
    f.owner.update(geometryUpdate(f, true, true, tx));
    assert.equal(f.jobs.length, 0);
    assert.equal(marker.removed, false);
    f.owner.keydown(key('Escape'));
    f.owner.refreshGeometry();
    assert.equal(f.jobs.length, 0);
    assert.equal(
      nodes.every((node) => node.removed),
      true,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('already-read marker cannot publish across a newer actual geometry notification', () => {
  const { f, nodes, shiftGeometry } = paintFixture();
  try {
    f.setChipGeometry(0, 15, 50);
    f.owner.pointerdown(pointer(1, 50, 15));
    drain(f);
    dragPointer(f.owner, pointer(1, 50, 15));
    drain(f);
    f.owner.refreshGeometry();
    const pending = f.jobs.shift()!,
      measured = pending.read(f.view),
      publication = f.state.field(reorderPreview).value,
      selection = f.state.selection;
    assert.ok(measured && typeof measured === 'object' && 'rect' in measured);
    assert.deepEqual(measured.rect, {
      left: 10,
      right: 12,
      top: 10,
      bottom: 30,
    });
    shiftGeometry(100, 0);
    f.owner.update(geometryUpdate(f, true, false));
    assert.equal(
      nodes.some((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed),
      false,
    );
    pending.write!(measured, f.view);
    assert.equal(
      nodes.some((n) => n.className === 'sp-reorder-insertion-marker' && !n.removed),
      false,
    );
    drain(f);
    const current = nodes.findLast(
      (n) => n.className === 'sp-reorder-insertion-marker' && !n.removed,
    )!;
    assert.equal(current.style.left, '110px');
    assert.equal(current.style.top, '10px');
    assert.equal(f.state.field(reorderPreview).value, publication);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
