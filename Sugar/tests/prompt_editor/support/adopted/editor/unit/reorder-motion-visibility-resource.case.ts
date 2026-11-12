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
/** Complete Resource admission validates actual CM mask output and exact descriptor freshness. */
import assert from 'node:assert/strict';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import test from 'node:test';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  currentReorderPaintTargets,
  ReorderWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { setReorderMotionMask } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import { ReorderMotionStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-styles.ts';
import { reorderMotionLocalAttribute } from '../../../../../../frontend/sugarprompt/editor/src/reorder-chip-presentation.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';

/** The native dispatch overload discriminates actual transactions and readonly batches before forwarding. */
function nativeDispatch(
  dispatch: EditorView['dispatch'],
  value: Transaction | TransactionSpec | readonly Transaction[],
): void {
  /** Array shape is the supported native batch discriminant; its typed elements remain unchanged. */
  const batch = (
    input: Transaction | TransactionSpec | readonly Transaction[],
  ): input is readonly Transaction[] => Array.isArray(input);
  if (value instanceof Transaction) dispatch(value);
  else if (batch(value)) dispatch(value);
  else dispatch(value);
}
/** The same actual presentation/scene/Widget owners use a literal observer subscription transport. */
function fixture() {
  let observer: ReturnType<typeof visibilityObserverFixture> | null = null;
  const f = motionPresentationFixture(undefined, (document) => {
    observer = visibilityObserverFixture(document);
  });
  assert.ok(observer);
  return {
    f,
    observer: observer as ReturnType<typeof visibilityObserverFixture>,
  };
}

/** Semantic cleanup drains the existing exact-issue deferred release, not a replacement scheduler. */
function releaseTasks(f: ReturnType<typeof motionPresentationFixture>): void {
  while (f.tasks.length) f.tasks.shift()!();
}

test('actual Resource accepts supported local metadata records without self-cancellation and still observes foreign mutation', () => {
  const { f, observer } = fixture();
  try {
    f.prepare();
    f.deliver();
    const mask = f.state.field(reorderPreview).mask;
    assert.ok(mask);
    assert.deepEqual(
      currentReorderPaintTargets(f.view)!
        .filter(
          (entry) => entry.kind === 'chip' && entry.node.hasAttribute(reorderMotionLocalAttribute),
        )
        .map((entry) => entry.index),
      mask.ids,
    );
    assert.equal(f.frames.size, 1);
    assert.equal(observer.active, 2);
    observer.deliver();
    assert.equal(f.state.field(reorderPreview).mask, mask);
    assert.equal(f.frames.size, 1);
    f.root.setAttribute('data-foreign', '1');
    observer.deliver();
    releaseTasks(f);
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.frames.size, 0);
    assert.equal(observer.active, 0);
  } finally {
    f.destroy();
  }
});

for (const frontier of ['receipt-getter', 'activation'] as const)
  test(`same-string newer issue at ${frontier} cannot activate or retain the old Resource`, () => {
    const { f, observer } = fixture();
    const originalActivate = ReorderMotionStyles.prototype.activate;
    let changed = false,
      newerIssue: object | null = null;
    /** Publish the same selected output under a genuinely newer StateField descriptor. */
    const replace = (): void => {
      const mask = f.state.field(reorderPreview).mask;
      if (!mask || changed) return;
      changed = true;
      newerIssue = {};
      f.view.dispatch({
        effects: setReorderMotionMask.of({ ...mask, issue: newerIssue }),
      });
    };
    try {
      if (frontier === 'activation')
        ReorderMotionStyles.prototype.activate = function (issue, current) {
          const activated = originalActivate.call(this, issue, current);
          if (activated) replace();
          return activated;
        };
      else
        for (const target of currentReorderPaintTargets(f.view)!.filter(
          (entry) => entry.kind === 'chip',
        )) {
          const get = target.node.getAttribute.bind(target.node);
          target.node.getAttribute = (name) => {
            const value = get(name);
            if (name === reorderMotionLocalAttribute && value !== null) replace();
            return value;
          };
        }
      f.prepare();
      f.deliver();
      releaseTasks(f);
      assert.equal(changed, true);
      assert.equal(f.state.field(reorderPreview).mask?.issue, newerIssue);
      assert.equal(f.frames.size, 0);
      assert.equal(observer.active, 0);
      assert.equal(f.refreshes, 0);
      assert.equal(
        f.view.dom
          .querySelector('[data-reorder-motion-active]')
          ?.getAttribute('data-reorder-motion-active') || '',
        '',
      );
    } finally {
      ReorderMotionStyles.prototype.activate = originalActivate;
      f.destroy();
    }
  });

for (const fault of ['same-attribute', 'class', 'style', 'child'] as const)
  test(`Resource rejects exact publication with extra ${fault} transport`, () => {
    const { f, observer } = fixture();
    f.prepare();
    f.deliver();
    assert.ok(f.state.field(reorderPreview).mask);
    f.publish();
    const publication = f.state.field(reorderPreview);
    assert.ok(publication.value);
    assert.equal(
      new ReorderWidget(publication.value, publication.mask).updateDOM(f.root, f.view),
      true,
    );
    const dispatch = f.view.dispatch.bind(f.view);
    let changed = false;
    f.view.dispatch = (spec) => {
      nativeDispatch(dispatch, spec);
      if (changed || !f.state.field(reorderPreview).mask) return;
      changed = true;
      const target = currentReorderPaintTargets(f.view)!.find(
        (entry) => entry.kind === 'chip' && entry.node.hasAttribute(reorderMotionLocalAttribute),
      )!.node;
      if (fault === 'same-attribute') {
        const value = target.getAttribute(reorderMotionLocalAttribute)!;
        target.setAttribute(reorderMotionLocalAttribute, 'foreign');
        target.setAttribute(reorderMotionLocalAttribute, value);
      } else if (fault === 'child') target.append(target.ownerDocument.createElement('span'));
      else target.setAttribute(fault, fault === 'style' ? 'opacity: 0.5;' : 'foreign');
    };
    try {
      f.prepare();
      f.deliver();
      releaseTasks(f);
      assert.equal(changed, true);
      assert.equal(f.frames.size, 0);
      assert.equal(observer.active, 0);
      assert.equal(f.refreshes, 1);
      assert.equal(f.presentation.retain(f.input), null);
      assert.equal(f.state.field(reorderPreview).mask, null);
    } finally {
      f.destroy();
    }
  });

test('preexisting pending native record is adjudicated before mask publication', () => {
  const { f, observer } = fixture();
  let changed = false;
  observer.onObserve((target) => {
    if (target === f.view.contentDOM && !changed) {
      changed = true;
      f.root.setAttribute('data-foreign', '1');
    }
  });
  try {
    f.prepare();
    f.deliver();
    releaseTasks(f);
    assert.equal(changed, true);
    assert.equal(f.frames.size, 0);
    assert.equal(observer.active, 0);
    assert.equal(f.state.field(reorderPreview).mask, null);
  } finally {
    f.destroy();
  }
});
test('foreign publication invalidation cannot release a newer Resource admitted by refresh', () => {
  const { f, observer } = fixture();
  const dispatch = f.view.dispatch.bind(f.view);
  let corrupted = false,
    renewed = false,
    oldIssue: object | null = null;
  f.view.dispatch = (spec) => {
    nativeDispatch(dispatch, spec);
    const mask = f.state.field(reorderPreview).mask;
    if (!mask || corrupted) return;
    corrupted = true;
    oldIssue = mask.issue;
    f.root.setAttribute('data-foreign', '1');
  };
  f.onRefresh(() => {
    if (renewed) return;
    renewed = true;
    // Deliver the already-issued old exact-mask release before the fresh native preparation phases.
    releaseTasks(f);
    f.publish();
    const field = f.state.field(reorderPreview);
    assert.ok(field.value);
    assert.equal(new ReorderWidget(field.value, field.mask).updateDOM(f.root, f.view), true);
    f.prepare();
  });
  try {
    f.prepare();
    f.deliver();
    releaseTasks(f);
    assert.equal(renewed, true);
    assert.equal(f.state.field(reorderPreview).mask, null);
    f.deliver();
    releaseTasks(f);
    const newer = f.state.field(reorderPreview).mask;
    assert.ok(newer);
    assert.notEqual(newer.issue, oldIssue);
    assert.equal(renewed, true);
    assert.equal(f.refreshes, 1);
    assert.equal(f.frames.size, 1);
    assert.equal(observer.active, 2);
    observer.deliver();
    assert.equal(f.state.field(reorderPreview).mask, newer);
  } finally {
    f.destroy();
  }
});

for (const raw of [null, undefined])
  test(`foreign publication refresh preserves raw ${String(raw)} after finite retirement`, () => {
    const { f, observer } = fixture();
    const dispatch = f.view.dispatch.bind(f.view);
    let changed = false;
    f.view.dispatch = (spec) => {
      nativeDispatch(dispatch, spec);
      if (!changed && f.state.field(reorderPreview).mask) {
        changed = true;
        f.root.setAttribute('data-foreign', '1');
      }
    };
    f.onRefresh(() => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw raw;
    });
    try {
      let failure: { readonly cause: unknown } | null = null;
      try {
        f.prepare();
        f.deliver();
      } catch (cause) {
        failure = { cause };
      }
      assert.ok(failure);
      assert.equal(failure.cause, raw);
      releaseTasks(f);
      assert.equal(observer.active, 0);
      assert.equal(f.frames.size, 0);
      assert.equal(f.state.field(reorderPreview).mask, null);
    } finally {
      f.destroy();
    }
  });
