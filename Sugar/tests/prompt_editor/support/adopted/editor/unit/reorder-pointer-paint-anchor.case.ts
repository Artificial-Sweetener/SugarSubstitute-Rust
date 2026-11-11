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
/** Actual StateField, Core and canonical Widget characterize semantic/projection separation under an issued primary hold. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  reorderPreview,
  installReorderPreview,
  publishReorderPreview,
  fallbackReorderPointerPaint,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { createReorderApplicationHold } from '../../../../../../frontend/sugarprompt/editor/src/reorder-session-application.ts';
import {
  setReorderMotionMask,
  releaseReorderMotionMask,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import {
  currentReorderPaintTargets,
  currentReorderPaintLease,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { currentReorderProjectionLease } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-projection.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  prepareReorderWidgetPlan,
  preparedReorderRowPaintAdmission,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';
import { fixture, key } from './reorder-session-fixture.ts';
import { drain } from './reorder-marker-fixture.ts';

/** The genuine keyboard pickup supplies a noninitial Core order; only publication is directly controlled here. */
function anchorFixture() {
  const f = fixture();
  f.owner.keydown(key('Alt'));
  drain(f);
  f.owner.keydown(key('ArrowRight'));
  drain(f);
  const pickup = f.state.field(reorderPreview).value!;
  assert.deepEqual(pickup.order.order, [1, 0, 2]);
  const hold = createReorderApplicationHold(pickup.order);
  const moved = moveReorderChip(pickup.capture.prepared, pickup.order, pickup.activeChip, {
    kind: 'row',
    row: 0,
    insertion: 2,
  });
  assert.ok(moved);
  /** Real Core output enters the same publication owner used by Session. */
  const publish = (order = moved, issued = hold) => {
    assert.equal(
      publishReorderPreview(f.view, { ...pickup, order }, 'pointer', issued, () => true),
      'published',
    );
    f.draw();
    return f.state.field(reorderPreview);
  };
  return { f, pickup, hold, moved, publish };
}

test('changed pointer proposals retain actual pickup Widget output, exact targets and source/history', () => {
  const { f, pickup, moved, publish } = anchorFixture();
  try {
    const first = publish(),
      root = f.paintedRoot,
      targets = currentReorderPaintTargets(f.view);
    assert.equal(first.pointer != null, true);
    assert.equal(first.pointer!.anchor.order === pickup.order, true);
    assert.equal(first.paint === first.pointer!.anchor, true);
    assert.equal(first.value!.order === moved, true);
    assert.equal(root != null && targets !== null && targets.length === 3, true);
    assert.equal(currentReorderPaintLease(f.view, first.value!) === null, true);
    assert.equal(currentReorderProjectionLease(f.view, first.value!)?.(), true);
    const second = publish(pickup.order);
    assert.equal(second.pointer === first.pointer, true);
    assert.equal(second.decorations === first.decorations, true);
    assert.equal(second.outer === first.outer, true);
    assert.equal(f.paintedRoot === root, true);
    assert.equal(currentReorderPaintTargets(f.view) === targets, true);
    assert.equal(readExactSource(f.state) === 'a, b, c', true);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('semantic replacement retires its composite lease while pickup paint remains mounted until exact Widget retirement', () => {
  const { f, pickup, publish } = anchorFixture(),
    state = f.state,
    source = readExactSource(state);
  try {
    const first = publish(),
      root = f.paintedRoot,
      mounted = currentReorderPaintLease(f.view, first.paint!),
      semantic = currentReorderProjectionLease(f.view, first.value!);
    assert.ok(root);
    assert.ok(mounted);
    assert.ok(semantic);
    assert.equal(mounted(), true);
    assert.equal(semantic(), true);

    const second = publish(pickup.order),
      current = currentReorderProjectionLease(f.view, second.value!);
    assert.notEqual(second.value, first.value);
    assert.equal(second.paint, first.paint);
    assert.equal(f.paintedRoot, root);
    assert.equal(semantic(), false);
    assert.equal(currentReorderProjectionLease(f.view, first.value!), null);
    assert.equal(mounted(), true);
    assert.ok(current);
    assert.equal(current(), true);

    f.owner.keydown(key('Escape'));
    f.draw();
    assert.equal(f.paintedRoot, null);
    assert.equal(mounted(), false);
    assert.equal(current(), false);
    assert.equal(currentReorderProjectionLease(f.view, second.value!), null);
    assert.equal(f.state.doc, state.doc);
    assert.equal(f.state.selection, state.selection);
    assert.equal(readExactSource(f.state), source);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test('whole coverage retains underlay during exact fallback and exposes current CM only on exact mask retirement', () => {
  const { f, pickup, publish } = anchorFixture();
  try {
    const first = publish(),
      issue = {},
      mask = {
        issue,
        scope: {},
        capture: pickup.capture,
        ids: [],
        coverage: 'value' as const,
      };
    f.view.dispatch({ effects: setReorderMotionMask.of(mask) });
    const covered = f.state.field(reorderPreview);
    assert.equal(covered.decorations === first.decorations, true);
    assert.equal(covered.outer === first.outer, true);
    assert.equal(
      fallbackReorderPointerPaint(f.view, first.pointer!, first.value!, () => true),
      true,
    );
    f.draw();
    const native = f.state.field(reorderPreview);
    assert.equal(native.pointer!.mode, 'fallback');
    assert.equal(native.paint === native.value, true);
    assert.equal(native.outer === covered.outer, true);
    f.view.dispatch({ effects: releaseReorderMotionMask.of(issue) });
    assert.equal(f.state.field(reorderPreview).outer.size, 0);
    assert.equal(publish(pickup.order).pointer!.mode, 'fallback');
  } finally {
    f.destroy();
  }
});

test('stale fallback cannot latch a superseding proposal or another hold', () => {
  const { f, pickup, moved, publish } = anchorFixture();
  try {
    const old = publish();
    let replaced = false;
    assert.equal(
      fallbackReorderPointerPaint(f.view, old.pointer!, old.value!, () => {
        if (!replaced) {
          replaced = true;
          publish(pickup.order);
        }
        return true;
      }),
      false,
    );
    assert.equal(f.state.field(reorderPreview).pointer!.mode, 'stage');
    const newer = publish(moved, createReorderApplicationHold(pickup.order));
    assert.equal(newer.pointer !== old.pointer, true);
    assert.equal(
      fallbackReorderPointerPaint(f.view, old.pointer!, old.value!, () => true),
      false,
    );
    assert.equal(f.state.field(reorderPreview).pointer === newer.pointer, true);
  } finally {
    f.destroy();
  }
});

/** A new hold's exact canonical plan is available before CM decides whether equivalent pickup DOM needs updating. */
test('sticky canonical pickup prepares the exact pointer anchor before native materialization and reuses its plan', () => {
  const { f, pickup, moved, hold } = anchorFixture();
  try {
    const root = f.paintedRoot;
    assert.ok(root);
    assert.equal(f.state.field(reorderPreview).native, false);
    assert.equal(
      publishReorderPreview(f.view, { ...pickup, order: moved }, 'pointer', hold, () => true),
      'published',
    );
    const first = f.state.field(reorderPreview),
      anchor = first.pointer!.anchor;
    assert.equal(first.paint, anchor);
    assert.notEqual(anchor, pickup);
    assert.equal(anchor.order, pickup.order);
    assert.equal(anchor.output.text, 'b, a, c');
    assert.equal(preparedReorderRowPaintAdmission(anchor), 'absent');
    const plan = prepareReorderWidgetPlan(anchor);
    assert.equal(plan.nativeRowPaint, false);
    assert.equal(f.paintedRoot, root);
    f.draw();
    assert.equal(prepareReorderWidgetPlan(anchor), plan);
    assert.equal(f.paintedRoot, root);
    assert.equal(
      publishReorderPreview(
        f.view,
        { ...pickup, order: pickup.order },
        'pointer',
        hold,
        () => true,
      ),
      'published',
    );
    assert.equal(f.state.field(reorderPreview).paint, anchor);
    assert.equal(prepareReorderWidgetPlan(anchor), plan);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

/** Delegated currentness can synchronously replace the publication after cold plan preparation; the older hold must never overwrite it. */
test('new canonical anchor preparation rejects stale and synchronously superseded publication before dispatch', () => {
  for (const mode of ['stale', 'superseded'] as const) {
    const { f, pickup, moved, hold } = anchorFixture();
    try {
      const before = f.state;
      let checks = 0;
      assert.equal(
        publishReorderPreview(f.view, { ...pickup, order: moved }, 'pointer', hold, () => {
          if (++checks === 1) return true;
          if (mode === 'superseded') f.view.dispatch({ effects: installReorderPreview.of(null) });
          return mode === 'superseded';
        }),
        'stale',
      );
      assert.equal(checks, 2);
      if (mode === 'stale') assert.equal(f.state, before);
      else assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  }
});
