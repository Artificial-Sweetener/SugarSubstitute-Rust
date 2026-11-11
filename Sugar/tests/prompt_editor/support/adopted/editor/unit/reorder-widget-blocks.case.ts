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
/** Real canonical updates retain complete immutable native blocks; controlled DOM transport makes no glyph or timing claim. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  ReorderWidget,
  currentReorderPaintTargets,
  currentReorderPaintLease,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { prepareReorderWidgetPlan } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';
import {
  reorderMountedIdentity,
  reorderMountedPlans,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-block.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { reorderState } from '../transaction/reorder-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';

for (const source of ['a, b, c', 'a, a, a', 'a, b\nc, d'])
  test(`canonical blocks retain every unchanged chip and distinct carrier occurrence for ${source.length} source units`, () => {
    const f = motionPresentationFixture(reorderState(source).state);
    try {
      const preview = f.state.field(reorderPreview).value!;
      const before = Array.from(f.root.childNodes);
      const originalTargets = currentReorderPaintTargets(f.view)!;
      const chips = new Map(
        originalTargets
          .filter((target) => target.kind === 'chip')
          .map((target) => [target.index, target.node]),
      );
      const nextOrder = moveReorderChip(preview.capture.prepared, preview.order, 0, {
        kind: 'row',
        row: 0,
        insertion: 0,
      });
      assert.ok(nextOrder);
      const next = prepareReorderPreview({ ...preview, order: nextOrder }, 'pointer');
      assert.ok(next);
      const plans = reorderMountedPlans(prepareReorderWidgetPlan(preview));
      const buckets = new Map<string, Node[]>();
      plans.forEach((plan, at) => {
        const key = reorderMountedIdentity(plan);
        const nodes = buckets.get(key) ?? [];
        nodes.push(before[at]!);
        buckets.set(key, nodes);
      });
      const nextWidget = new ReorderWidget(next, null);
      assert.equal(nextWidget.updateDOM(f.root, f.view), true);
      const nextTargets = currentReorderPaintTargets(f.view)!;
      assert.equal(f.root.textContent, next.output.text);
      const gaps = Array.from(f.root.childNodes).filter(
        (node) => (node as HTMLElement).className === 'sp-reorder-gap',
      );
      assert.equal(gaps.length, nextOrder.order.length + 1);
      assert.equal(new Set(gaps).size, gaps.length);
      for (const target of nextTargets.filter((target) => target.kind === 'chip'))
        assert.equal(target.node, chips.get(target.index));
      const nextPlans = reorderMountedPlans(prepareReorderWidgetPlan(next));
      nextPlans.forEach((plan, at) => {
        const retained = buckets.get(reorderMountedIdentity(plan))?.shift();
        if (retained) assert.equal(f.root.childNodes[at], retained);
      });
      assert.equal(new Set(f.root.childNodes).size, f.root.childNodes.length);
      assert.deepEqual(
        nextTargets.filter((target) => target.kind === 'chip').map((target) => target.index),
        nextOrder.order,
      );
      const metadata = nextPlans.flatMap((plan) =>
        plan.kind === 'chip'
          ? [plan.parts]
          : plan.kind === 'gap'
            ? plan.segments.filter((segment) => segment.marker).map((segment) => segment.parts)
            : [],
      );
      assert.deepEqual(
        nextTargets.map((target) => target.origins),
        metadata.map((parts) =>
          parts.flatMap((part) => (part.kind === 'line' ? [part.identity] : [])),
        ),
      );
      assert.equal(readExactSource(f.state), source);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('same-order held-class publication performs no native child mutations', () => {
  const f = motionPresentationFixture();
  try {
    const preview = f.state.field(reorderPreview).value!;
    const before = Array.from(f.root.childNodes);
    const originals = new Map<Node, () => void>();
    for (const node of before) {
      const child = node as HTMLElement;
      originals.set(child, child.remove);
      child.remove = () => assert.fail('unchanged-child-removed');
    }
    const insert = f.root.insertBefore;
    f.root.insertBefore = () => assert.fail('same-order-child-inserted');
    const next = prepareReorderPreview({ ...preview, activeChip: 2 }, 'pointer');
    assert.ok(next);
    try {
      assert.equal(new ReorderWidget(next, null).updateDOM(f.root, f.view), true);
      assert.deepEqual(Array.from(f.root.childNodes), before);
      const held = currentReorderPaintTargets(f.view)!.find(
        (target) => target.kind === 'chip' && target.index === 2,
      )!;
      assert.equal(held.node.className, 'sp-reorder-chip sp-reorder-chip-active sp-reorder-held');
    } finally {
      f.root.insertBefore = insert;
      for (const [node, remove] of originals) (node as HTMLElement).remove = remove;
    }
  } finally {
    f.destroy();
  }
});

test('retained rich media preserves its complete wrapper, line and actual prepared canvas across a canonical move', () => {
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, 'document');
  const f = motionPresentationFixture(
    preparedLoraState(() => {}),
    (document) => {
      Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: document,
      });
    },
  );
  try {
    const preview = f.state.field(reorderPreview).value!;
    const target = currentReorderPaintTargets(f.view)!.find(
      (target) => target.kind === 'chip' && target.index === 1,
    )!;
    const children = Array.from(target.node.childNodes);
    const canvases = Array.from(target.node.querySelectorAll('canvas'));
    assert.ok(canvases.length > 0);
    const order = moveReorderChip(preview.capture.prepared, preview.order, 0, {
      kind: 'row',
      row: 0,
      insertion: 0,
    });
    assert.ok(order);
    const next = prepareReorderPreview({ ...preview, order }, 'complete');
    assert.ok(next);
    assert.equal(new ReorderWidget(next, null).updateDOM(f.root, f.view), true);
    const retained = currentReorderPaintTargets(f.view)!.find(
      (target) => target.kind === 'chip' && target.index === 1,
    )!;
    assert.equal(retained.node, target.node);
    assert.deepEqual(Array.from(retained.node.childNodes), children);
    assert.deepEqual(Array.from(retained.node.querySelectorAll('canvas')), canvases);
  } finally {
    try {
      f.destroy();
    } finally {
      if (previousDocument) Object.defineProperty(globalThis, 'document', previousDocument);
      else Reflect.deleteProperty(globalThis, 'document');
    }
  }
});

test('same-preview nested canonical update retires only the older issue and its captured geometry lease', () => {
  const f = motionPresentationFixture();
  try {
    const preview = f.state.field(reorderPreview).value!;
    const lease = currentReorderPaintLease(f.view, preview);
    assert.ok(lease);
    assert.equal(lease(), true);
    const chip = currentReorderPaintTargets(f.view)![0]!.node;
    const contains = chip.contains;
    let nested = false;
    chip.contains = (node) => {
      chip.contains = contains;
      nested = new ReorderWidget(preview, null).updateDOM(f.root, f.view);
      return contains.call(chip, node);
    };
    assert.equal(new ReorderWidget(preview, null).updateDOM(f.root, f.view), false);
    assert.equal(nested, true);
    assert.equal(lease(), false);
    assert.equal(f.root.textContent, preview.output.text);
    assert.equal(
      currentReorderPaintTargets(f.view)!.filter((target) => target.kind === 'chip').length,
      3,
    );
  } finally {
    f.destroy();
  }
});

test('an unfinished wrapper cannot be retained by a nested update during its first class assignment', () => {
  const f = motionPresentationFixture();
  try {
    const preview = f.state.field(reorderPreview).value!;
    const chip = currentReorderPaintTargets(f.view)!.find((target) => target.kind === 'chip')!.node;
    (chip.childNodes[0] as HTMLElement).remove();
    const doc = f.view.dom.ownerDocument;
    const create = doc.createElement.bind(doc);
    let nested = false,
      armed = true;
    doc.createElement = ((name: string) => {
      const node = create(name);
      if (armed) {
        armed = false;
        let value = node.className;
        Object.defineProperty(node, 'className', {
          configurable: true,
          get: () => value,
          set: (next) => {
            value = next;
            Object.defineProperty(node, 'className', {
              configurable: true,
              writable: true,
              value,
            });
            nested = new ReorderWidget(preview, null).updateDOM(f.root, f.view);
          },
        });
      }
      return node;
    }) as Document['createElement'];
    assert.equal(new ReorderWidget(preview, null).updateDOM(f.root, f.view), false);
    assert.equal(nested, true);
    assert.equal(f.root.textContent, preview.output.text);
    const targets = currentReorderPaintTargets(f.view)!.filter((target) => target.kind === 'chip');
    assert.equal(targets.length, 3);
    assert.ok(targets.every((target) => target.node.textContent!.length > 0));
  } finally {
    f.destroy();
  }
});

test('changed-block retirement preserves raw failure and still retires all other borrowed paint', () => {
  const f = motionPresentationFixture();
  try {
    const preview = f.state.field(reorderPreview).value!;
    const targets = currentReorderPaintTargets(f.view)!;
    const chip = targets.find((target) => target.kind === 'chip')!.node;
    (chip.childNodes[0] as HTMLElement).remove();
    chip.remove = () => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw null;
    };
    let failed = false;
    try {
      new ReorderWidget(preview, null).updateDOM(f.root, f.view);
    } catch (cause) {
      failed = true;
      assert.equal(cause, null);
    }
    assert.equal(failed, true);
    assert.equal(currentReorderPaintTargets(f.view), null);
    for (const target of targets)
      for (const lease of target.leases) assert.equal(lease.node.isConnected, false);
  } finally {
    f.destroy();
  }
});
